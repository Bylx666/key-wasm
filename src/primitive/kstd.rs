//! 定义顶级作用域的函数

use crate::intern::{Interned,intern};
use crate::primitive::litr::{Litr, Function};
use crate::runtime::{calc::CalcRef, Scope, Variant};

pub fn prelude()-> Vec<Variant> {
  macro_rules! prel {($($name:literal:$f:ident)*)=>{
    vec![$( Variant {
      name: intern($name),
      locked: true,
      v: Litr::Func(Function::Native($f))
    }, )*]
  }}
  prel!{
    b"log":log
    b"throw":throw
    b"run_ks":run_ks
    b"version":version
    b"distribution":distribution
    b"swap":swap
    b"take":take
  }
}

/// 输出到控制台
pub fn log(args:Vec<CalcRef>, _cx:Scope)-> Litr {
  args.iter().for_each(|v|
    crate::log(v.str().as_bytes()));
  Litr::Uninit
}

/// 手动报错
pub fn throw(args:Vec<CalcRef>, _cx:Scope)-> Litr {
  let s = args.get(0).map_or("错误".to_string(),|s|s.str());
  panic!("{s}");
}

/// 在当前作用域 解析并运行一段String
pub fn run_ks(args:Vec<CalcRef>, mut cx:Scope)-> Litr {
  let s = args.get(0).expect("evil需要传入一个被解析的字符串或数组");
  let s = match &**s {
    Litr::Str(s)=> s.as_bytes(),
    Litr::Buf(b)=> &**b,
    _=> panic!("evil只能运行字符串或数组")
  };

  unsafe {
    // 将报错位置写为evil 并保存原先的报错数据
    let mut place = std::mem::take(&mut crate::PLACE);
    let line = crate::LINE;
    crate::PLACE = format!("run_ks: {}({})", place, line);
    crate::LINE = 1;

    // 解析并运行
    let scanned = crate::scan::scan(s);
    for (l, sm) in &scanned.0 {
      unsafe{
        println!("{l}");
        crate::LINE = *l;
      }
      cx.evil(sm);
      // 如果evil到return或break就在这停下
      if cx.ended {
        break;
      }
    }

    // 还原报错信息
    crate::PLACE = std::mem::take(&mut place);
    crate::LINE = line;
  }
  Litr::Uninit
}

/// 获取版本号
fn version(_a:Vec<CalcRef>, _c:Scope)-> Litr {
  Litr::Uint(crate::VERSION)
}

/// 获取发行者
fn distribution(_a:Vec<CalcRef>, _c:Scope)-> Litr {
  Litr::Str(crate::DISTRIBUTION.to_string())
}

/// 无分配的直接交互数值
fn swap(mut args:Vec<CalcRef>, _c:Scope)-> Litr {
  assert!(args.len()>=2, "swap需要两个值用于无分配交换");
  let mut it = args.iter_mut();
  std::mem::swap(&mut **it.next().unwrap(), &mut **it.next().unwrap());
  Litr::Uninit
}

/// 无分配的直接取走一个值, 并将原值变为uninit
fn take(mut args:Vec<CalcRef>, _c:Scope)-> Litr {
  let a = args.get_mut(0).expect("take需要一个被取走的值");
  let mut b = Litr::Uninit;
  std::mem::swap(&mut **a, &mut b);
  b
}