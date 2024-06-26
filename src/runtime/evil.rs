use crate::primitive;
use std::cmp::Ordering;

use super::*;

impl Scope {
  /// 解析一个语句，对应Stmt
  pub fn evil(&mut self, code:&Stmt) {
    match code {
      // 只有表达式的语句
      Stmt::Expression(e)=> {
        // 如果你只是在一行里空放了一个变量就不会做任何事
        if let Expr::Variant(_) = e {
          return;
        }
        self.calc_ref(e);
      }


      // let语句
      Stmt::Let(asn)=> assign(*self, asn, false),
      // const语句
      Stmt::Const(asn)=> assign(*self, asn, true), 
      /// 锁定语句
      Stmt::Lock(id)=> self.lock(*id),

      // 块语句
      Stmt::Block(s)=> self.subscope().run(s),

      // 类型声明
      Stmt::Class(raw)=> {
        let clsdef = ClassDef { p:*raw, cx:self.clone() };
        self.class_defs.push(clsdef);
        let using = self.class_defs.last_mut().unwrap() as *mut ClassDef;
        self.class_uses.push((unsafe{(**raw).name}, Class::Local(using)));
      }

      Stmt::Using(alia, e)=> {
        match e {
          Expr::Variant(id)=> {
            let cls = self.find_class(*id).unwrap_or_else(||panic!("未定义类'{}'", id.str()));
            self.class_uses.push((*alia, cls));
          }
          Expr::ModClsAcc(s, modname)=> {
            let cls = self.find_class_in(*s, *modname);
            self.class_uses.push((*alia, cls));
          }
          _=> panic!("class = 语句后必须是个类声明")
        }
      }
      
      // 导入模块
      Stmt::Mod(name, m)=> unsafe {
        (*self.imports).push((*name, Module::Local(*m)));
      }

      // 导出函数 mod.
      Stmt::ExportFn(id, f)=> {
        // 将函数本体生命周期拉为static
        let func_raw = Box::leak(Box::new(f.clone()));
        let f = LocalFunc::new(func_raw, *self);
        // 将函数定义处的作用域生命周期永久延长
        outlive::increase_scope_count(f.scope);
        self.vars.push(Variant {
          name:*id, v:Litr::Func(Function::Local(f.clone())), locked:false
        });
        unsafe{(*self.exports).funcs.push((*id,f))}
      }

      // 导出类 mod:
      Stmt::ExportCls(raw)=> {
        // 将class的定义复制一份, 因为其scan的结果会在模块运行完被drop
        let name = unsafe{(**raw).name};
        let cls = Box::into_raw(Box::new(raw.clone()));
        // 延长作用域生命周期
        outlive::increase_scope_count(*self);
        
        let clsdef = ClassDef { p:*raw, cx:self.clone() };
        self.class_defs.push(clsdef);
        let using = self.class_defs.last_mut().unwrap() as *mut ClassDef;
        self.class_uses.push((name, Class::Local(using)));
        
        // 将指针推到export
        let module = unsafe {&mut*self.exports};
        module.classes.push((name, using))
      }

      // 返回一个值
      Stmt::Return(expr)=> {
        // 遇到return语句就停止当前遍历
        // 并将返回值指针相同(在同一函数内的作用域)设为已结束
        unsafe{*self.return_to = self.calc(expr)};
        self.ended = true;
        let mut scope = *self;
        while let Some(mut s) = scope.parent {
          if s.return_to != self.return_to {break;}
          s.ended = true;
          scope = s;
        }
      },

      // if else
      Stmt::If { condition, exec, els }=> {
        if cond(self.calc(condition)) {
          self.evil(exec)
        }else if let Some(els) = els {
          self.evil(els)
        }
      }

      // for ()语句
      Stmt::ForWhile { condition, exec }=>
        start_loop(*self, ||cond(self.calc(condition)), exec),

      // for!语句
      Stmt::ForLoop(exec)=> start_loop(*self, ||true, exec),

      // for v:iter语句
      Stmt::ForIter{exec, id, iterator: iter}=> {
        use primitive::iter::LitrIterator;

        let mut iter_ = self.calc_ref(iter);
        let mut iter = LitrIterator::new(&mut iter_);
        let mut breaked = false;

        // 记忆上次运行的作用域的变量数量
        let mut capa = 0;
        match &**exec {
          Stmt::Block(exec)=> {
            if let Some(first_var) = iter.next() {
              // 先运行一次测试出来变量数量
              {
                let mut scope = self.subscope();
                if let Some(id) = id {
                  scope.vars.push(Variant {name:*id, v:first_var, locked:false});
                }
                loop_run(scope, &mut breaked, exec);
                capa = scope.vars.len();
                outlive::scope_end(scope);
              }

              for v in iter {
                let mut scope = self.subscope();
                scope.vars = Vec::with_capacity(capa);
                if scope.ended || breaked {
                  outlive::scope_end(scope);
                  return;
                }
                if let Some(id) = id {
                  scope.vars.push(Variant {name:*id, v, locked:false});
                }
                loop_run(scope, &mut breaked, exec);
                outlive::scope_end(scope);
              }
            }
          },

          // 禁止单语句直接用循环控制语句
          Stmt::Break=> panic!("不允许`for v:iter break`的写法"),
          Stmt::Continue=> panic!("不允许`for v:iter continue`的写法`"),
          
          // 单语句运行
          _=> if let None = id {
            let mut scope = self.subscope();
            for v in iter {
              self.evil(exec);
            }
            outlive::scope_end(scope);
          }else {
            // 指定迭代过程的变量名时不可使用单语句写法
            panic!("指定了变量名的迭代 不可使用单语句")
          }
        }
      },

      Stmt::Match{ to, arms, def }=> {
        let to = self.calc_ref(to);
        // 将Ordering和MatchOrd对比
        let matcher = |(val, ord):&(Expr, MatchOrd)|
         match (&*to).partial_cmp(&*self.calc_ref(val)) {
          Some(Ordering::Equal)=> match ord {
            MatchOrd::Eq|MatchOrd::GreaterEq|MatchOrd::LessEq=> true,
            _=> false
          }
          Some(Ordering::Greater)=> match ord {
            MatchOrd::Greater|MatchOrd::GreaterEq=> true,
            _=> false
          }
          Some(Ordering::Less)=> match ord {
            MatchOrd::Less|MatchOrd::LessEq=> true,
            _=> false
          }
          None=> false
        };

        for (conds, stmts) in arms {
          // 如果第一个条件的符号是=就是逻辑或(any, 任意符合)
          let matched = if let MatchOrd::Eq = conds[0].1 {
            conds.iter().any(matcher)
          }else {
          // 如果是大于小于就是逻辑与(all, 全部符合)
            conds.iter().all(matcher)
          };
          // 匹配并运行
          if matched {
            self.subscope().run(stmts);
            return;
          };
        }
        // 运行默认语句
        if let Some(def) = def {
          self.subscope().run(def);
        }
      }
      
      Stmt::Throw(s)=> panic!("{}", self.calc_ref(s).str()),
      Stmt::Try { stmt, catc }=> {
        // 感受wasm的魅力吧)呕
        static mut TRYING: &Stmt = &Stmt::Empty;
        static mut TRYING_SELF: Scope = Scope {ptr:std::ptr::null_mut()};
        static mut CATCHING_NAME: Interned = unsafe{std::mem::transmute(0)};
        static mut CATCHING: &Statements = &Statements(Vec::new());
        static mut EMPTY_STMTS:Statements = Statements(Vec::new());

        #[link(wasm_import_module = "key")]
        extern {
          fn key_try(t:fn(), c:fn(*mut u8, usize));
        }

        fn tryer() {
          unsafe {TRYING_SELF.evil(TRYING)}
        }
        fn catcher(s:*mut u8, l:usize) {
          unsafe {
            let s = unsafe{String::from_utf8_unchecked(Vec::from_raw_parts(s, l, l))};
            let mut scope = TRYING_SELF.subscope();
            scope.vars.push(Variant { name:CATCHING_NAME, locked: false, v:  Litr::Str(s)});
            scope.run(CATCHING);
          }
        }
        
        unsafe{
          let stmt: &'static Stmt = unsafe{std::mem::transmute(&**stmt)};
          TRYING = stmt;
          TRYING_SELF = *self;
          if let Some((name, stmts)) = catc {
            let stmts: &'static Statements = unsafe{std::mem::transmute(stmts)};
            CATCHING_NAME = *name;
            CATCHING = stmts;
            key_try(tryer, catcher)
          }else {
            CATCHING_NAME = intern(b".err");
            CATCHING = &EMPTY_STMTS;
            key_try(tryer, catcher)
          }
        }
      }

      // -
      Stmt::Break=> panic!("break不在循环体内"),
      Stmt::Continue=> panic!("continue不在循环体内"),
      Stmt::Empty=> (),
    }
  }
}

/// let和const
fn assign(mut s:Scope, asn:&AssignDef, locked: bool) {
  // 如果用的是<而不是=, 则直接夺取右侧值所有权
  let mut v = if asn.take {
    std::mem::take(&mut *s.calc_ref(&asn.val))
  } else {
    s.calc(&asn.val)
  };
  // 不检查变量是否存在是因为寻找变量的行为是反向的
  match &asn.id {
    AssignTo::One(id)=> {
      s.vars.push(Variant {name:*id, v, locked});
    }
    AssignTo::Destr(ids)=> match v {
      // 属性解构
      Litr::Inst(mut inst)=> {
        let cls = unsafe{&*inst.cls};
        for id in ids {
          let i = cls.props.iter()
            .position(|prop|*id==prop.name)
            .expect(&format!("本地类'{}'实例没有'{}'属性", cls.name, id));
          let v = std::mem::take(&mut inst.v[i]);
          s.vars.push(Variant {name:*id, v, locked});
        }
      }
      Litr::Ninst(v)=> {
        let cls = unsafe{&*v.cls};
        for id in ids {
          let v = (cls.getter)(&v, *id);
          s.vars.push(Variant {name:*id, v, locked});
        }
      }
      Litr::Obj(mut map)=> for id in ids {
        let v = map.remove(id).expect(&format!("哈希表中没有'{id}'属性"));
        s.vars.push(Variant {name:*id, v, locked});
      }

      // 线性解构
      Litr::Buf(v)=> {
        let mut v = v.into_iter();
        for id in ids {
          let v = v.next().map_or(Litr::Uninit, |n|Litr::Uint(n as _));
          s.vars.push(Variant {name:*id, v, locked});
        }
      }
      Litr::Str(str)=> {
        let mut itr = str.chars();
        for id in ids {
          let v = itr.next().map_or(Litr::Uninit, |s|Litr::Str(s.to_string()));
          s.vars.push(Variant {name:*id, v, locked});
        }
      }
      Litr::List(v)=> {
        let mut v = v.into_iter();
        for id in ids {
          let v = v.next().unwrap_or(Litr::Uninit);
          s.vars.push(Variant {name:*id, v, locked});
        }
      }
      _=> panic!("{v:?}无法被解构赋值")
    }
  }
}

/// 判断if后的条件
fn cond(v:Litr)-> bool {
  match v {
    Litr::Bool(b)=> b,
    Litr::Uninit=> false,
    _=> panic!("条件必须为Bool或uninit")
  }
}

/// 在一个作用域开始循环
fn start_loop(mut this:Scope, mut condition:impl FnMut()-> bool, exec:&Box<Stmt>) {
  // 用重置作用域代替重新创建作用域
  if let Stmt::Block(exec) = &**exec {
    let mut scope = this.subscope();
    let mut breaked = false;
    while condition() {
      if scope.ended || breaked {
        outlive::scope_end(scope);
        return;
      }
      // 重置此作用域
      scope.vars.clear();
      scope.class_uses.clear();
      loop_run(scope, &mut breaked, exec);
    }
    outlive::scope_end(scope);
  // 单语句将由当前作用域代为执行,不再创建新作用域
  }else {
    match &**exec {
      Stmt::Break=> panic!("不允许`for() break`的写法"),
      Stmt::Continue=> panic!("不允许`for() continue`的写法`"),
      _=> while condition() {
        if this.ended {
          return;
        }
        this.evil(exec);
      }
    }
  }
}

/// 以循环模式运行一段语句
fn loop_run(mut scope:Scope,breaked:&mut bool,exec:&Statements) {
  // 对于单Stmt的run实现
  macro_rules! loop_run_stmt {($stmt:expr)=>{{
    match $stmt {
      Stmt::Block(exec)=> {
        let mut s = scope.subscope();
        loop_run(s, breaked, exec);
        s.ended = true;
        outlive::scope_end(s);
      },
      Stmt::Break=> return *breaked = true,
      Stmt::Continue=> return,
      _=> scope.evil($stmt)
    };
  }}}

  for (l, sm) in &exec.0 {
    // 如果中途遇到return或者break就停止
    if scope.ended || *breaked {
      return;
    }
    match sm {
      Stmt::Break=> return *breaked = true,
      Stmt::Continue=> return,
      // 把直属该for下的块拦截,检测break和continue
      Stmt::Block(v)=> loop_run(scope, breaked, exec),
      Stmt::If { condition, exec, els }=> {
        if cond(scope.calc(condition)) {
          loop_run_stmt!(&**exec)
        }else if let Some(els) = els {
          loop_run_stmt!(&**els)
        }
      },
      _=> {
        unsafe{LINE = *l;}
        scope.evil(sm);
      }
    }
  }
}