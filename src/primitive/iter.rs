use std::cell::UnsafeCell;

use crate::{
  intern::intern, native::{NativeInstance, NativeMethod}, primitive::litr::{Litr, LocalFunc}, runtime::{calc::CalcRef, Scope}
};

use super::sym::Symbol;

/// instance类型专用的迭代器
struct InstanceIter<'a> {
  f: LocalFunc,
  kself: &'a mut Litr
}
impl Iterator for InstanceIter<'_> {
  type Item = Litr;
  fn next(&mut self) -> Option<Self::Item> {
    let r = Scope::call_local_with_self(&self.f, vec![], self.kself);
    if let Litr::Sym(s) = &r {
      if let Symbol::IterEnd = s {
        return None;
      }
    }
    Some(r)
  }
}

/// native instance专用的iter
struct NativeInstanceIter<'a> {
  f: fn(&mut NativeInstance)-> Litr,
  kself: &'a mut NativeInstance
}
impl Iterator for NativeInstanceIter<'_> {
  type Item = Litr;
  fn next(&mut self) -> Option<Self::Item> {
    let r = (self.f)(self.kself);
    if let Litr::Sym(s) = &r {
      if let Symbol::IterEnd = s {
        return None;
      }
    }
    Some(r)
  }
}

pub struct LitrIterator<'a> {
  inner: Box<dyn Iterator<Item = Litr> + 'a>
}
impl<'a> LitrIterator<'a> {
  pub fn new(v:&'a mut Litr)-> Self {
    let inner:Box<dyn Iterator<Item = Litr>> = match v {
      Litr::Str(s)=> Box::new(s.chars().map(|c|Litr::Str(c.to_string()))),
      Litr::Buf(v)=> Box::new(v.iter().map(|n|Litr::Uint((*n) as usize))),
      Litr::Uint(n)=> Box::new((0..*n).into_iter().map(|n|Litr::Uint(n))),
      Litr::Int(n)=> Box::new((0..*n).into_iter().map(|n|Litr::Int(n))),
      Litr::List(v)=> Box::new(v.iter().cloned()),
      Litr::Inst(inst)=> {
        let f = & unsafe{&*inst.cls}.methods.iter()
          .find(|f|f.name == intern(b"@next"))
          .expect("迭代class需要定义'.@next()'方法").f;
        let f = LocalFunc::new(f, unsafe{&*inst.cls}.cx);
        Box::new(InstanceIter { f, kself:v })
      }
      Litr::Ninst(inst) => {
        let f = unsafe {&*inst.cls}.next;
        Box::new(NativeInstanceIter {f, kself:inst})
      },
      Litr::Obj(o) => Box::new(o.keys()
        .map(|n|Litr::Str(unsafe{String::from_utf8_unchecked(n.vec().to_vec())}))),
      Litr::Bool(_) => panic!("Bool无法迭代"),
      Litr::Func(_) => panic!("Func无法迭代"),
      Litr::Float(_) => panic!("Float无法迭代"),
      Litr::Sym(_) => panic!("Sym无法迭代"),
      Litr::Uninit => panic!("给uninit迭代?死刑!"),
    };
    LitrIterator { inner }
  }
}

impl<'a> Iterator for LitrIterator<'a> {
  type Item = Litr;
  fn next(&mut self) -> Option<Self::Item> {
    self.inner.next()
  }
}
