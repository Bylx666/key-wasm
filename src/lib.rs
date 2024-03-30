#![allow(unused)]
#![allow(improper_ctypes_definitions)]
#![allow(improper_ctypes)]

use std::{fs, collections::HashMap, mem::transmute, hash::{BuildHasher, Hash}, vec};
use std::process::ExitCode;

mod intern;
mod scan;
mod runtime;
mod primitive;
mod native;


#[link(wasm_import_module = "key")]
extern {
  /// 浏览器的fetch, 发起网络请求
  /// 传入字符串的指针和长度, 并传进接受fetch结果的字符的数据的回调函数
  fn fetch(s_ptr:*const u8, s_len:usize, f:fn(s_ptr:*mut u8,s_len:usize));
  /// 传入字符串并打印
  fn log(s_ptr:*const u8, s_len:usize);
  /// 传入字符串并throw
  fn err(s_ptr:*const u8, s_len:usize);
}

/// 标志目前走到的行号
static mut LINE:usize = 1;
/// 用于标记报错文件
static mut PLACE:String = String::new();

/// 标志解释器的版本
static VERSION:usize = 100000;

/// 解释器发行者(用于区分主版本和魔改版)
/// 
/// 如果需要自己魔改,且需要考虑和主版本的兼容性可以更改此值
/// 
/// 用户可以使用distribution()直接读取此值
static DISTRIBUTION:&str = "Subkey";

#[no_mangle]
extern fn init() {
  intern::init();
  std::panic::set_hook(Box::new(|inf|{
    let line = unsafe{LINE};
    let place = unsafe{&*PLACE};
    let s = if let Some(mes) = inf.payload().downcast_ref::<&'static str>() {
      mes
    }else if let Some(mes) = inf.payload().downcast_ref::<String>() {
      mes
    }else{"错误"};
    let s = format!("\n> {}\n  {}:第{}行\n\n> Key Script CopyLeft by Subkey", s, place, line);
    unsafe {err(s.as_ptr(), s.len())}
  }));
}

#[no_mangle]
extern fn run() {
  // 自定义报错
  unsafe {PLACE = "global".to_string()}

  let scanned = scan::scan(b"");
  runtime::run(&scanned);
}

#[no_mangle]
extern fn fe() {
  fn cb(p:*mut u8, len:usize) {
    let s = unsafe{String::from_raw_parts(p, len, len)};
    unsafe{log(s.as_ptr(),s.len());}
  }
  let s:&'static str = "/sample/a.ks";
  unsafe{fetch(s.as_ptr(), s.len(), cb);}
}

#[no_mangle]
/// js端传入字符长度, rust分配好后返回指针
/// 然后js就会在指针上写入字符串数据
extern fn alloc(len:usize)-> *mut u8 {
  unsafe {std::alloc::alloc(std::alloc::Layout::from_size_align_unchecked(len, 1))}
}

macro_rules! def_calling {{$(
  $n:ident(
    $($args:ident$(,)?)*
  )
)*} => {
  $(
    #[no_mangle]
    extern fn $n(f:fn($($args:usize,)*),$($args:usize,)*) {
      f($($args,)*)
    }
  )*
}}
def_calling!{
  call1(a)
  call2(a,b)
}