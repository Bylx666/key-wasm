let utf8 = {
  parse: TextDecoder.prototype.decode.bind(new TextDecoder()),
  from: TextEncoder.prototype.encode.bind(new TextEncoder())
}
let _wasm = null;
let wasm = {
  get mem() {
    return _wasm.memory.buffer;
  },
  /// 读取指针处字符串
  read(ptr, len) {
    return utf8.parse(new Uint8Array(wasm.mem,ptr,len));
  },
  /// 写入arrayBuffer
  write(v) {
    let len = v.byteLength;
    let p = _wasm.alloc(len);
    new Uint8Array(wasm.mem).set(new Uint8Array(v), p);
    return p;
  }
}

WebAssembly.instantiateStreaming(
  fetch("/target/wasm32-unknown-unknown/debug/key_wasm.wasm"),
  {
    key: {
      log(ptr,len) {
        console.log(wasm.read(ptr,len))
      },
      err(ptr,len) {
        console.error(wasm.read(ptr, len))
      },
      key_try(run, catc) {
        try {
          _wasm.call0(run)
        }catch (e) {
          let s = utf8.from(e+"");
          _wasm.call2(catc, wasm.write(s), s.byteLength);
        }
      },
      // 暂时无用
      fetch_str(ptr,len) {
        // 读取字符串并fetch
        let req = new XMLHttpRequest();
        // 活久见, 这玩意居然有第三个参数代表同步
        req.open("GET", wasm.read(ptr, len), false);
        req.send();
        let buf = utf8.from(req.response);
        // 写入buffer并告诉rust写入的字符串信息
        let writed = wasm.write(buf);
        _wasm.set_str(writed, buf.byteLength);
      },
      // wasm获取mod的行为, 需要自行替换
      // 见key-play仓库
      fetch_mod(ptr, len) {
        _wasm.set_str(0,0);
      }
    }
  }
).then(v=> {
  _wasm = v.instance.exports;
  _wasm.init();
  return fetch("/sample/a.ks").then(v=>v.arrayBuffer())
}).then(v=> {
  let p = wasm.write(v);
  _wasm.run(p, v.byteLength);
});

// print_ast

// ).then(v=> {
//   _wasm = v.instance.exports;
//   _wasm.init();
//   return fetch("/sample/a.ks").then(v=>v.arrayBuffer())
// }).then(v=> {
//   let p = wasm.write(v);
//   _wasm.run(p, v.byteLength);
// });