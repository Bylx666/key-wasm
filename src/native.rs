//! 提供Native Module的接口

use crate::{
  intern::{intern, Interned}, 
  scan::stmt::LocalMod,
  primitive::litr::Litr
};
use crate::runtime::{calc::CalcRef, Scope};

pub type NativeFn = fn(Vec<CalcRef>, Scope)-> Litr;
pub type NativeMethod = fn(&mut NativeInstance, args:Vec<CalcRef>, Scope)-> Litr;

#[derive(Debug, Clone)]
pub struct NativeMod {
  pub funcs: Vec<(Interned, NativeFn)>,
  pub classes: Vec<*mut NativeClassDef>
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct NativeClassDef {
  pub name: Interned,
  pub statics: Vec<(Interned, NativeFn)>,
  pub methods: Vec<(Interned, NativeMethod)>,
  pub getter: fn(&NativeInstance, get:Interned)-> Litr,
  pub setter: fn(&mut NativeInstance, set:Interned, to:Litr),
  pub index_get: fn(&NativeInstance, CalcRef)-> Litr,
  pub index_set: fn(&mut NativeInstance, CalcRef, Litr),
  pub next: fn(&mut NativeInstance)-> Litr,
  pub to_str: fn(&NativeInstance)-> String,
  pub onclone: fn(&NativeInstance)-> NativeInstance,
  pub ondrop: fn(&mut NativeInstance)
}

/// 原生类型实例
#[derive(Debug)]
#[repr(C)]
pub struct NativeInstance {
  pub v: usize,
  pub w: usize,
  pub cls: *mut NativeClassDef,
}
impl Clone for NativeInstance {
  /// 调用自定义clone (key-native库中的默认clone行为也可用)
  fn clone(&self) -> Self {
    (unsafe{&*self.cls}.onclone)(self)
  }
}
impl Drop for NativeInstance {
  /// 调用自定义drop (key-native的默认drop不做任何事)
  fn drop(&mut self) {
    (unsafe{&*self.cls}.ondrop)(self)
  }
}
