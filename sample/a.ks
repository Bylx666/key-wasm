
//mod /sample/b.ks> m;


let inner() {
  class A{};
  let a = 20;
  let b = 40;
}

{
  inner.unzip();
  log(a, b, A::{});
  // 20 40 60
}
:90