let utf8 = {
  parse: TextDecoder.prototype.decode.bind(new TextDecoder()),
  from: TextEncoder.prototype.encode.bind(new TextEncoder())
}
let wasm = null;

WebAssembly.instantiateStreaming(
  fetch("/target/wasm32-unknown-unknown/debug/key_wasm.wasm"),
  {
    key: {
      log(ptr,len) {
        console.log(utf8.parse(new Uint8Array(wasm.memory.buffer,ptr,len)))
      },
      err(ptr,len) {
        throw(utf8.parse(new Uint8Array(wasm.memory.buffer,ptr,len)))
      },
      fetch(ptr,len,callback) {
        // 将作为fetch参数的字符串解析成js字符串
        let s = utf8.parse(new Uint8Array(wasm.memory.buffer,ptr,len));
        fetch(s).then(s=>s.arrayBuffer()).then(buf=> {
          // 分配空间
          let alloced = wasm.alloc(buf.byteLength);
          // 写入结果
          new Uint8Array(wasm.memory.buffer).set(new Uint8Array(buf), alloced);
          // 该回调函数接受2个参数, 调用2参数版call, 并传入buf指针和长度
          wasm.call2(callback, alloced, buf.byteLength);
        });
      }
    }
  }
).then(v=> {
  wasm = v.instance.exports;
  wasm.fe();
});