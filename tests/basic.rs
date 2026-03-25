#![allow(unused_imports)]
use cyclotrace::*;

#[cfg(test)]
#[cfg(feature = "loom")]
mod loom {
    use super::*;
    use ::loom;

    #[test]
    fn test_loom() {
        loom::model(|| {
            let (writer, reader) = create_buffer::<u32, 2>();
            let t1 = loom::thread::spawn(move || {
                writer.write(42);
            });
            let r1 = reader.clone();
            let t2 = loom::thread::spawn(move || {
                if let Some(v) = r1.get(0) {
                    assert_eq!(v, 42);
                }
            });
            t1.join().unwrap();
            t2.join().unwrap();
            let v = reader.get(0);
            assert_eq!(v, Some(42));
        });
    }

    #[test]
    fn get_consistency() {
        loom::model(|| {
            let (writer, reader) = create_buffer::<u32, 2>();

            let t = loom::thread::spawn(move || {
                writer.write(1);
                writer.write(2);
            });

            if let Some(v) = reader.get(0) {
                assert!(v == 1 || v == 2);
            }

            if let Some(v) = reader.get(1) {
                assert!(v == 1 || v == 2);
            }

            t.join().unwrap();
        });
    }

    #[test]
    fn wraparound() {
        loom::model(|| {
            let (writer, reader) = create_buffer::<u32, 2>();

            let t = loom::thread::spawn(move || {
                writer.write(1);
                writer.write(2);
                writer.write(3);
            });

            for i in 0..3 {
                if let Some(v) = reader.get(i) {
                    assert!(v <= 3);
                }
            }

            t.join().unwrap();
        });
    }

    #[test]
    fn multiple_get() {
        loom::model(|| {
            let (writer, reader) = create_buffer::<u32, 2>();

            let t = loom::thread::spawn(move || {
                writer.write(42);
            });

            let r1 = reader.clone();
            let t1 = loom::thread::spawn(move || {
                if let Some(v) = r1.get(0) {
                    assert_eq!(v, 42);
                }
            });
            let r2 = reader;
            let t2 = loom::thread::spawn(move || {
                if let Some(v) = r2.get(0) {
                    assert_eq!(v, 42);
                }
            });

            t.join().unwrap();
            t1.join().unwrap();
            t2.join().unwrap();
        });
    }

    #[test]
    fn range_get() {
        loom::model(|| {
            let (writer, reader) = create_buffer::<u32, 4>();

            let t1 = loom::thread::spawn(move || {
                writer.write(1);
                writer.write(2);
                writer.write(3);
            });

            let t2 = loom::thread::spawn(move || {
                for _ in 0..3 {
                    let mut buf = Vec::new();
                    if reader.get_range(.., &mut buf).is_some() {
                        assert!(buf.iter().all(|&x| x <= 3));
                    }
                }
            });

            t1.join().unwrap();
            t2.join().unwrap();
        });
    }

    #[test]
    fn read() {
        loom::model(|| {
            let (writer, reader) = create_buffer::<u32, 4>();

            let t1 = loom::thread::spawn(move || {
                writer.write(1);
                writer.write(2);
                writer.write(3);
            });

            let t2 = loom::thread::spawn(move || {
                let v = reader.read_latest();

                assert!(v <= 3);
            });

            t1.join().unwrap();
            t2.join().unwrap();
        })
    }

    #[test]
    fn read_range() {
        loom::model(|| {
            let (writer, reader) = create_buffer::<u32, 4>();
            let t1 = loom::thread::spawn(move || {
                writer.write(1);
                writer.write(2);
                writer.write(3);
            });

            let t2 = loom::thread::spawn(move || {
                let mut buf = Vec::new();
                reader.read_range(.., &mut buf).unwrap();
                assert!(buf.iter().all(|&x| x <= 3));
            });

            t1.join().unwrap();
            t2.join().unwrap();
        });
    }
}

#[cfg(test)]
#[cfg(not(feature = "loom"))]
mod tests {
    use super::*;

    #[test]
    fn multiple_get() {
        let (writer, reader) = create_buffer::<u32, 2>();
        writer.write(42);
        assert_eq!(reader.get(0), Some(42));
        assert_eq!(reader.get(0), Some(42));
    }

    #[test]
    fn empty() {
        let (_, reader) = create_buffer::<u32, 2>();
        assert_eq!(reader.get(0), None);
    }

    #[test]
    fn get() {
        let (writer, reader) = create_buffer::<u32, 2>();
        writer.write(42);
        assert_eq!(reader.get(0), Some(42));
        assert_eq!(reader.get(1), None);
    }

    #[test]
    fn wraparound() {
        let (writer, reader) = create_buffer::<u32, 4>();
        writer.write(1);
        writer.write(2);
        writer.write(3);
        assert_eq!(reader.get(0), Some(3));
        assert_eq!(reader.get(1), Some(2));
    }

    #[test]
    fn range_get() {
        let (writer, reader) = create_buffer::<u32, 4>();
        writer.write(1);
        writer.write(2);
        writer.write(3);
        let mut buf = Vec::new();
        let _ = reader.get_range(.., &mut buf);
        assert_eq!(buf, vec![1, 2, 3]);
    }

    #[test]
    fn range_get_wraparound() {
        let (writer, reader) = create_buffer::<u32, 4>();
        writer.write(1);
        writer.write(2);
        writer.write(3);
        writer.write(4);
        writer.write(5);
        let mut buf = Vec::new();
        let _ = reader.get_range(.., &mut buf);
        assert_eq!(buf, vec![3, 4, 5]);
    }

    #[test]
    fn range_get_empty() {
        let (_writer, reader) = create_buffer::<u32, 2>();
        let mut buf = Vec::new();
        let _ = reader.get_range(.., &mut buf);
        assert!(buf.is_empty());
    }

    #[test]
    fn range_get_slice() {
        let (writer, reader) = create_buffer::<u32, 4>();
        writer.write(1);
        writer.write(2);
        writer.write(3);
        let mut buf = [0; 3];
        let _ = reader.get_range(.., buf.as_mut_slice());
        assert_eq!(buf, [1, 2, 3]);
    }

    #[test]
    fn range_get_array() {
        let (writer, reader) = create_buffer::<u32, 4>();
        writer.write(1);
        writer.write(2);
        writer.write(3);
        let mut buf = [0; 3];
        let _ = reader.get_range(.., &mut buf);
        assert_eq!(buf, [1, 2, 3]);
    }

    #[test]
    fn read_latest() {
        let (writer, reader) = create_buffer::<u32, 4>();
        writer.write(42);
        assert_eq!(reader.read_latest(), 42);
    }

    #[test]
    fn read_latest_across_threads() {
        let (writer, reader) = create_buffer::<u32, 4>();

        let t = std::thread::spawn(move || {
            writer.write(42);
        });

        assert_eq!(reader.read_latest(), 42);

        t.join().unwrap();
    }

    #[test]
    fn read_range() {
        let (writer, reader) = create_buffer::<u32, 4>();
        writer.write(1);
        writer.write(2);
        writer.write(3);
        let mut buf = Vec::new();
        reader.read_range(.., &mut buf).unwrap();
        assert_eq!(buf, vec![1, 2, 3]);
    }

    #[test]
    fn read_overwrite() {
        let (writer, reader) = create_buffer::<u32, 4>();
        writer.write(1);
        writer.write(2);
        writer.write(3);
        writer.write(4);
        writer.write(5);
        let mut buf = Vec::new();
        reader.read_range(.., &mut buf).unwrap();
        assert_eq!(buf, vec![3, 4, 5]);
    }

    #[cfg(feature = "arrayvec")]
    #[test]
    fn range_get_arrayvec() {
        let (writer, reader) = create_buffer::<u32, 4>();
        writer.write(1);
        writer.write(2);
        writer.write(3);
        let mut buf = arrayvec::ArrayVec::<u32, 3>::new();
        let _ = reader.get_range(.., &mut buf);
        assert_eq!(buf, arrayvec::ArrayVec::<u32, 3>::from([1, 2, 3]));
    }

    #[cfg(feature = "heapless")]
    #[test]
    fn range_get_heapless() {
        let (writer, reader) = create_buffer::<u32, 4>();
        writer.write(1);
        writer.write(2);
        writer.write(3);
        let mut buf = heapless::Vec::<u32, 3>::new();
        let _ = reader.get_range(.., &mut buf);
        assert_eq!(buf, heapless::Vec::<u32, 3>::from([1, 2, 3]));
    }
}
