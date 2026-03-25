use core::hint::black_box;
use std::thread;

use cyclotrace::*;

#[derive(Clone, Copy)]
struct BigData {
    _big: [u64; 16],
}

fn main() {
    let (writer, reader) = create_buffer::<BigData, 64>();

    let h1 = thread::spawn(move || {
        writer.write(BigData { _big: [0; 16] });
    });

    let h2 = thread::spawn(move || {
        black_box(reader.get(0));
    });
    h1.join().unwrap();
    h2.join().unwrap();
}
