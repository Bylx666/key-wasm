use super::{Scanner, scan};
use crate::intern::{Interned,intern};
use crate::runtime::{Scope, ScopeInner, Module};
use crate::LINE;
use crate::primitive::litr::{
  Litr, Function, LocalFuncRaw, LocalFunc, ExternFunc, KsType
};
use crate::scan::Expr;

/// 语句列表
#[derive(Debug, Clone, Default)]
pub struct Statements (
  pub Vec<(usize, Stmt)>
);

/// 分号分隔的，statement语句
#[derive(Debug, Clone)]
pub enum Stmt {
  Empty,

  // 赋值
  Let       (AssignDef),
  Const     (AssignDef),
  // 锁定变量
  Lock      (Interned),

  // 定义类
  Class     (*const ClassDefRaw),
  // 类别名
  Using     (Interned, Expr),

  Mod       (Interned, *const LocalMod),
  ExportFn  (Interned, LocalFuncRaw),
  ExportCls (*const ClassDefRaw),

  Match {
    to: Expr,
    arms: Vec<(Vec<(Expr, MatchOrd)>, Statements)>,
    def: Option<Statements>
  }, 

  // 块系列
  Block    (Statements),   // 一个普通块
  If {
    condition: Expr,
    exec: Box<Stmt>,
    els: Option<Box<Stmt>>
  },
  ForLoop(Box<Stmt>),
  ForWhile {
    condition: Expr,
    exec: Box<Stmt>
  },
  ForIter {
    iterator: Expr,
    id: Option<Interned>,
    exec: Box<Stmt>
  },

  // 流程控制
  Break,
  Continue,                           // 立刻进入下一次循环
  Return    (Expr),                  // 函数返回

  Throw (Expr),
  Try {
    stmt: Box<Stmt>,
    catc: Option<(Interned, Statements)>
  },

  // 表达式作为语句
  Expression(Expr),
}

/// 赋值语句
#[derive(Debug, Clone)]
pub struct AssignDef {
  /// =左侧
  pub id: AssignTo,
  /// =右侧
  pub val: Expr,
  /// 是否使用<代替=
  pub take: bool
}

#[derive(Debug, Clone)]
pub enum AssignTo {
  /// 单体赋值
  One(Interned),
  /// 解构赋值
  Destr(Vec<Interned>)
}


#[derive(Debug, Clone)]
pub struct LocalMod {
  pub funcs: Vec<(Interned, LocalFunc)>,
  pub classes: Vec<(Interned, *mut ClassDef)>
}

/// 未绑定作用域的类声明
#[derive(Debug, Clone)]
pub struct ClassDefRaw {
  pub name: Interned,
  pub props: Vec<ClassProp>,
  pub methods: Vec<ClassFuncRaw>,
  pub statics: Vec<ClassFuncRaw>
}

/// 绑定作用域的类声明
#[derive(Debug, Clone)]
pub struct ClassDef {
  pub p: *const ClassDefRaw,
  /// 代表该本地函数的上下文
  /// 用来判断是否在模块外,
  /// 如果属性使用了自定义class, 也会以此作用域寻找该class
  pub cx: Scope
}
impl std::ops::Deref for ClassDef {
  type Target = ClassDefRaw;
  fn deref(&self) -> &Self::Target {
    unsafe{&*self.p}
  }
}

/// 类中的属性声明
#[derive(Debug, Clone)]
pub struct ClassProp {
  pub name: Interned,
  pub typ: KsType,
  pub public: bool
}

/// 类中的未绑定作用域的函数声明
#[derive(Debug,Clone)]
pub struct ClassFuncRaw {
  pub name: Interned,
  pub f: LocalFuncRaw,
  pub public: bool
}

/// 类中的函数声明
#[derive(Debug,Clone)]
pub struct ClassFunc {
  pub name: Interned,
  pub f: LocalFunc,
  pub public: bool
}

#[derive(Debug, Clone)]
pub enum MatchOrd {
  Greater,
  GreaterEq,
  Less,
  LessEq,
  Eq
}

impl Scanner<'_> {
  /// 匹配一个语句
  pub fn stmt(&self)-> Stmt {
    self.spaces();
    if self.i() >= self.src.len() {
      self.next(); // 打破scan函数的while
      return Stmt::Empty;
    }

    let first = self.cur();
    match first {
      // 分号开头即为空语句
      b';' => {
        self.next();
        return Stmt::Empty;
      }
      // 块语句
      b'{' => {
        let mut stmts = Statements::default();
        let len = self.src.len();
        self.next();
        loop {
          assert!(self.i()<len, "未闭合的块大括号");
          self.spaces();
          if self.cur() == b'}' {
            self.next();
            return Stmt::Block(stmts);
          }
          
          let s = self.stmt();
          if let Stmt::Empty = s {
            continue;
          }
          stmts.0.push((unsafe{LINE}, s));
        }
      }
      // 返回语句语法糖
      b':' => {
        self.next();
        return self.returning();
      }
      // 修复开头遇到没定义的符号时死循环
      127..=u8::MAX|b')'|b'}'|b']'|b'?'|b','|b'\\'|b'$'|b'#'=> panic!("需要一个语句或表达式,但你写了'{}'",String::from_utf8_lossy(&[first])),
      _=>{}
    }

    let ident = self.literal();
    if let Expr::Variant(id) = ident {
      match &*id.vec() {
        // 如果是关键词，就会让对应函数处理关键词之后的信息
        b"let"=> Stmt::Let(self.letting()),
        b"const"=> {
          if self.cur()==b'(' {
            if let Expr::Variant(n) = self.expr_group() {
              Stmt::Lock(n)
            }else {panic!("const()锁定语句只允许传入变量名")}
          }else {
            Stmt::Const(self.letting())
          }
        },
        b"extern"=> panic!("wams extern暂未实装"),
        b"return"=> self.returning(),
        b"class"=> self.classing(),
        b"mod"=> self.moding(),
        b"for"=> self.foring(),
        b"if"=> self.ifing(),
        b"else"=> panic!("else必须紧接if. 检查一下是不是if后是单语句还用了分号结尾"),
        b"break"=> Stmt::Break,
        b"continue"=> Stmt::Continue,
        b"async"|b"await"=> panic!("异步关键词暂未实现"),
        b"throw"=> self.throwing(),
        b"try"=> self.trying(),
        b"catch"=> panic!("catch必须在try之后"),
        b"match"=> self.matching(),
        _=> {
          let expr = self.expr_with_left(ident, vec![]);
          Stmt::Expression(expr)
        }
      }
    }else if let Expr::Empty = ident {
      let expr = self.expr();
      if let Expr::Empty = expr {
        Stmt::Empty
      }else {
        Stmt::Expression(expr)
      }
    } else {
      let expr = self.expr_with_left(ident, vec![]);
      if let Expr::Empty = expr {
        Stmt::Empty
      }else {
        Stmt::Expression(expr)
      }
    }
  }

  /// 解析let关键词
  fn letting(&self)-> AssignDef {
    self.spaces();

    let id = match self.cur() {
      n@b'['|n@b'{'=> {
        self.next();
        let mut vec = Vec::new();
        self.spaces();
        while let Some(id) = self.ident() {
          vec.push(intern(id));
          self.spaces();
          if self.cur() == b',' {
            self.next();
            self.spaces();
          }
        }
        if n + 2 != self.cur() {
          panic!("let解构错误:未闭合的括号'{}'", String::from_utf8_lossy(&[n]))
        }
        self.next();
        AssignTo::Destr(vec)
      }
      _=> AssignTo::One(intern(self.ident().unwrap_or_else(||panic!("let后需要标识符"))))
    };
  
    // 检查标识符后的符号
    self.spaces();
    let sym = self.cur();
    match sym {
      b'=' => {
        self.next();
        let val = self.expr();
        if let Expr::Empty = val {
          panic!("无法为空气赋值")
        }
        AssignDef {
          id, val, take:false
        }
      }
      // take语法
      b'<'=> {
        self.next();
        let val = self.expr();
        if let Expr::Empty = val {
          panic!("无法为空气赋值")
        }
        AssignDef {
          id, val, take:true
        }
      }
      b'(' => {
        self.next();
        let args = self.arguments();
        assert!(self.cur()==b')', "函数声明右括号缺失");
        self.next();
  
        let stmt = self.stmt();
        let mut stmts = if let Stmt::Block(b) = stmt {
          b
        }else {
          Statements(vec![(unsafe{LINE}, stmt)])
        };
  
        // scan过程产生的LocalFunc是没绑定作用域的，因此不能由运行时来控制其内存释放
        // 其生命周期应当和Statements相同，绑定作用域时将被复制
        // 绑定作用域行为发生在runtime::Scope::calc
        AssignDef {
          id, take:false,
          val: Expr::LocalDecl(LocalFuncRaw { argdecl: args, stmts })
        }
      }
      _ => AssignDef {
        id, take:false, val:Expr::Literal(Litr::Uninit)
      }
    }
  }
  
  // extern关键词
  // wasm版暂未实装
  // fn externing(&self) {
  //   // 截取路径
  //   self.spaces();
  //   let mut i = self.i();
  //   let len = self.src.len();
  //   while self.src[i] != b'>' {
  //     assert!(i<len, "extern后需要 > 符号");
  //     i += 1;
  //   }
  
  //   let path = &self.src[self.i()..i];
  //   self.set_i(i + 1);
  //   self.spaces();
  
  //   /// 解析并推走一个函数声明
  //   macro_rules! parse_decl {($id:ident) => {{
  //     let sym:&[u8];
  //     // 别名解析
  //     if self.cur() == b':' {
  //       self.next();
  //       self.spaces();
  //       if let Some(i) = self.ident() {
  //         sym = i;
  //       }else {
  //         panic!(":后需要别名")
  //       };
  //     }else {
  //       sym = $id;
  //     }
  
  //     // 解析小括号包裹的参数声明
  //     assert!(self.cur()==b'(', "extern函数后应有括号");
  //     self.next();
  //     let argdecl = self.arguments();
  //     self.spaces();
  //     assert!(self.cur() == b')', "extern函数声明右括号缺失");
  //     self.next();
      
  //     if self.cur() == b';' {
  //       self.next();
  //     }
  //   }}}

  //   // 大括号语法
  //   if self.cur() == b'{' {
  //     self.next();
  //     self.spaces();
  //     while let Some(id) = self.ident() {
  //       parse_decl!(id);
  //       self.spaces();
  //     }
  //     self.spaces();
  //     assert!(self.cur() == b'}', "extern大括号未闭合");
  //     self.next();
  //   }else {
  //     // 省略大括号语法
  //     let id = self.ident().unwrap_or_else(||panic!("extern后应有函数名"));
  //     parse_decl!(id);
  //   }
  // }

  /// 解析返回语句
  fn returning(&self)-> Stmt {
    self.spaces();
    let expr = self.expr();
    if let Expr::Empty = expr {
      Stmt::Return(Expr::Literal(Litr::Uninit))
    }else {
      Stmt::Return(expr)
    }
  }

  /// 解析类声明
  fn classing(&self)-> Stmt {
    self.spaces();
    let id = self.ident().unwrap_or_else(||panic!("class后需要标识符"));
    self.spaces();
    if self.cur() == b'=' {
      self.next();
      let right = self.expr();
      return Stmt::Using(intern(id),right);
    }

    assert!(self.cur() == b'{', "class需要大括号");
    self.next();
  
    let mut props = Vec::new();
    let mut methods = Vec::new();
    let mut statics = Vec::new();
    loop {
      self.spaces();
      let public = if self.cur() == b'>' {
        self.next();self.spaces();true
      }else {false};
      
      let is_method = if self.cur() == b'.' {
        self.next();true
      }else {false};
  
      let id = match self.ident() {
        Some(id)=> id,
        None=> break
      };

      // 方法或者函数
      if self.cur() == b'(' {
        self.next();
        // 参数
        let args = self.arguments();
        assert!(self.cur() == b')', "函数声明右括号缺失");
        self.next();
  
        // 函数体
        let stmt = self.stmt();
        let mut stmts = if let Stmt::Block(b) = stmt {
          b
        }else {
          Statements(vec![(unsafe{LINE}, stmt)])
        };
  
        let v = ClassFuncRaw {name: intern(id), f:LocalFuncRaw{argdecl:args,stmts}, public};
        if is_method {
          methods.push(v);
        }else {
          statics.push(v);
        }
      // 属性
      }else {
        let typ = self.typ();
        let v = ClassProp {
          name: intern(id), typ, public
        };
        props.push(v);
      }
  
      self.spaces();
      if self.cur() == b',' {
        self.next();
      }
      self.spaces();
    }
  
    self.spaces();
    assert!(self.cur()==b'}', "class大括号未闭合");
    self.next();
    Stmt::Class(Box::into_raw(Box::new(ClassDefRaw {
      name:intern(id), props, methods, statics
    })))
  }
  
  
  /// 解析模块声明
  fn moding(&self)-> Stmt {
    // 先判断是否是导出语句
    match self.cur() {
      b'.' => {
        self.next();
        // 套用let声明模板
        let asn = self.letting();
        let id = if let AssignTo::One(n) = asn.id {n}else {
          panic!("mod.语句一次只能导出一个函数");
        };
        if let Expr::LocalDecl(f) = asn.val {
          return Stmt::ExportFn(id, f.clone());
        }
        panic!("模块只能导出本地函数。\n  若导出外界函数请用本地函数包裹。")
      },
      b':' => {
        self.next();
        let cls = self.classing();
        match cls {
          Stmt::Class(cls)=> return Stmt::ExportCls(cls),
          Stmt::Using(_,_)=> panic!("无法导出using"),
          _=> unreachable!()
        }
      }
      _=>{}
    };
    // 截取路径
    self.spaces();
    let mut i = self.i();
    let len = self.src.len();
    let mut dot = 0;
    loop {
      assert!(i<len, "mod后需要 > 符号");
      let cur = self.src[i];
      if cur == b'>' {
        break;
      }
      if cur == b'.' {
        dot = i;
      }
      i += 1;
    }

    let path:String = String::from_utf8_lossy(&self.src[self.i()..i]).into_owned();
    self.set_i(i + 1);

    self.spaces();
    let name = intern(&self.ident().unwrap_or_else(||panic!("需要为模块命名")));
    self.spaces();

    let file = crate::fetch_mod(path.as_bytes());

    unsafe {
      // 将报错位置写为该模块 并保存原先的报错数据
      let mut place = std::mem::take(&mut crate::PLACE);
      crate::PLACE = path.clone();
      let line = crate::LINE;
      crate::LINE = 1;

      let mut module = crate::runtime::run(&scan(file.as_bytes())).exports;

      // 还原报错信息
      crate::PLACE = std::mem::take(&mut place);
      crate::LINE = line;
      
      Stmt::Mod(name, module)
    }
  }

  fn ifing(&self)-> Stmt {
    let condition = self.expr();
    let exec = Box::new(self.stmt());
    self.spaces();
    if self.cur() == b'e' {
      let else_end = self.i() + 4;
      if else_end <= self.src.len() && &self.src[self.i()..else_end] == b"else" {
        self.set_i(else_end);
        let els = Some(Box::new(self.stmt()));
        return Stmt::If { condition, exec, els };
      }
    }
    Stmt::If { condition, exec, els: None }
  }

  /// for语句
  fn foring(&self)-> Stmt {
    self.spaces();
    match self.cur() {
      b'('=> {
        let condition = self.expr_group();
        let exec = Box::new(self.stmt());
        Stmt::ForWhile { condition, exec }
      }
      b'!'=> {
        self.next();
        let exec = Box::new(self.stmt());
        Stmt::ForLoop(exec)
      }
      _=> {
        let left = self.literal();
        self.spaces();
        // 使用迭代器值
        if self.cur() == b':' {
          self.next();
          if let Expr::Variant(id) = left {
            let right = self.expr();
            let exec = Box::new(self.stmt());
            return Stmt::ForIter {iterator:right, id:Some(id), exec};
          }
          panic!("`for v:iter`语句中:左边必须是标识符")
        }

        // 不使用迭代器值
        let iterator = match left {
          Expr::Empty=> self.expr(),
          _=> self.expr_with_left(left, vec![])
        };
        let exec = Box::new(self.stmt());
        Stmt::ForIter {iterator, id:None, exec}
      }
    }
  }
  
  /// throw
  fn throwing(&self)-> Stmt {
    self.spaces();
    let expr = self.expr();
    if let Expr::Empty = expr {
      panic!("throw后需要一个表达式")
    }
    Stmt::Throw(expr)
  }

  /// try catch
  fn trying(&self)-> Stmt {
    let block = Box::new(self.stmt());

    self.spaces();
    if self.cur() == b'c' {
      let catch_end = self.i() + 5;
      if catch_end <= self.src.len() && &self.src[self.i()..catch_end] == b"catch" {
        self.set_i(catch_end);
        self.spaces();
        let id = intern(self.ident().unwrap_or(b".err"));
        let catc = match self.stmt() {
          Stmt::Block(b)=> b,
          _=> panic!("catch之后必须是错误变量名和块语句")
        };
        return Stmt::Try { stmt: block, catc:Some((id, catc)) };
      }
    }

    Stmt::Try { stmt: block, catc: None }
  }
  
  /// 模式匹配语法
  fn matching(&self)-> Stmt {
    self.spaces();
    if self.cur() == b'{' {
      panic!("match后必须有表达式")
    }
    let to = self.expr();
    if let Expr::Empty = &to {
      panic!("match后必须有表达式")
    }
    self.spaces();

    assert!(self.cur()==b'{', "match表达式后必须有大括号");
    self.next();

    // 匹配条件和语句
    let mut arms = Vec::new();
    let mut def = None;
    'arm: loop {
      // 匹配条件
      let mut conds = Vec::new();
      loop {
        self.spaces();
        let mut ord = MatchOrd::Eq;

        match self.cur() {
          // 判断是否结束
          b'}'=> {
            self.next();
            break 'arm;
          }
          // 判断是否默认语句
          b'-'=> {
            self.next();
            self.spaces();
            assert!(self.cur()==b'{', "match默认语句必须是块语句");
            let run = if let Stmt::Block(stmt) = self.stmt() {stmt}else {
              unreachable!();
            };
            def = Some(run);
            continue 'arm;
          }
          // 判断大于小于前缀
          b'>'=> {
            self.next();
            ord = if self.cur() == b'=' {
              self.next();
              MatchOrd::GreaterEq
            }else {MatchOrd::Greater}
          }
          b'<'=> {
            self.next();
            ord = if self.cur() == b'=' {
              self.next();
              MatchOrd::LessEq
            }else {MatchOrd::Less}
          }
          b'='=> self.next(),
          _=> ()
        }

        // 非默认语句
        let e = self.expr();
        if let Expr::Empty = &e {
          panic!("match条件不可留空")
        }
        conds.push((e, ord));
        self.spaces();
        
        if self.cur() != b',' {break;}
        self.next();
      }

      assert!(self.cur()==b'{', "match条件后必须是'{{'");
      let run = if let Stmt::Block(stmt) = self.stmt() {stmt}else {
        unreachable!();
      };
      
      arms.push((conds, run))
    }

    Stmt::Match { to, arms, def }
  }
}

