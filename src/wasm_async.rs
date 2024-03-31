//! Wasm 的纯wasm sleep实现
//! 
//! 见 https://github.com/WebAssembly/binaryen/blob/main/src/passes/Asyncify.cpp
//! 
//! Rust实现由Subkey首发

#[derive(Debug)]
pub enum AsyncState {
  Normal,
  Unwind,
  Rewind
}

static mut ASYNC_STATE:AsyncState = AsyncState::Normal;

