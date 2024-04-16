#![allow(unused)]
#![feature(hash_set_entry)]
#![allow(improper_ctypes_definitions)]
#![allow(improper_ctypes)]

use std::{fs, collections::HashMap, mem::transmute, hash::{BuildHasher, Hash}, vec};
use std::process::ExitCode;

mod intern;
mod scan;
mod runtime;
mod primitive;
mod native;
// mod wasm_async;

/// 打印到控制台
fn log(s:&[u8]) {
  #[link(wasm_import_module = "key")]
  extern {
    fn log(s_ptr:*const u8, s_len:usize);
  }
  unsafe {log(s.as_ptr(), s.len())}
}

/// 手动报错
fn err(s:&[u8])->! {
  #[link(wasm_import_module = "key")]
  extern {
    fn err(s_ptr:*const u8, s_len:usize)->!;
  }
  unsafe {err(s.as_ptr(), s.len())}
}

/// 使用fetch从网络读取字符串
fn fetch_str(s:&[u8])-> String {
  #[link(wasm_import_module = "key")]
  extern {
    fn fetch_str(s_ptr:*const u8, s_len:usize);
  }
  // SAFETY: 写FFI哪有safe的
  unsafe {
    fetch_str(s.as_ptr(), s.len());
    let (ptr,len) = NEXT_STR;
    String::from_raw_parts(ptr, len, len)
  }
}

/// 标志目前走到的行号
static mut LINE:usize = 1;
/// 用于标记报错文件
static mut PLACE:String = String::new();

/// 标志解释器的版本
static VERSION:usize = 100006;

/// 解释器发行者(用于区分主版本和魔改版)
/// 
/// 如果需要自己魔改,且需要考虑和主版本的兼容性可以更改此值
/// 
/// 用户可以使用distribution()直接读取此值
static DISTRIBUTION:&str = "Subkey";

static mut PRINT_AST:bool = false;
#[no_mangle]
extern fn switch_print_ast(b:usize) {
  unsafe{match b {
    0=> PRINT_AST = false,
    _=> PRINT_AST = true
  }}
}

static PANIC_HOOK: fn(&std::panic::PanicInfo) = |inf|{
  let line = unsafe{LINE};
  let place = unsafe{&*PLACE};
  let s = if let Some(mes) = inf.payload().downcast_ref::<&'static str>() {
    mes
  }else if let Some(mes) = inf.payload().downcast_ref::<String>() {
    mes
  }else{"错误"};
  let s = format!("\n> {}\n  {}:第{}行\n\n> Key Script CopyLeft by Subkey", s, place, line);
  unsafe {err(s.as_bytes())}
};

#[no_mangle]
extern fn init() {
  intern::init();
  std::panic::set_hook(Box::new(PANIC_HOOK));
}

#[no_mangle]
extern fn run(p:*mut u8, len:usize) {
  /// 该指针是js端调用rust alloc得到的, 要在此拿到其所有权
  let s = unsafe { Vec::<u8>::from_raw_parts(p, len, len) };

  unsafe {PLACE = "用户输入".to_owned()}

  let scanned = scan::scan(&s);
  if unsafe{PRINT_AST} {
    log(format!("{:?}", scanned).as_bytes())
  }
  runtime::run(&scanned);
}

#[no_mangle]
/// js端传入字符长度, rust分配好后返回指针
/// 然后js就会在指针上写入字符串数据
extern fn alloc(len:usize)-> *mut u8 {
  unsafe {std::alloc::alloc(std::alloc::Layout::from_size_align_unchecked(len, 1))}
}

static mut NEXT_STR:(*mut u8, usize) = (std::ptr::null_mut(), 0);
#[no_mangle]
unsafe extern fn set_str(p:*mut u8, len:usize) {
  NEXT_STR = (p,len);
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
  call0()
  call1(a)
  call2(a,b)
}
