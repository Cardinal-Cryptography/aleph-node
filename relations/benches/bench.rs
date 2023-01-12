//
// cargo bench
//

use criterion::{criterion_group, criterion_main, Criterion};
use relations::preimage_proving;

fn preimage(c: &mut Criterion) {
    c.bench_function("preimage", |f| f.iter(preimage_proving));
}

criterion_group!(benches, preimage);
criterion_main!(benches);
