#![allow(unused_imports)]
use cyclotrace::*;

#[cfg(test)]
#[cfg(feature = "loom")]
mod loom {
    use super::*;
    use ::loom;

    #[test]
    fn try_read() {
        loom::model(|| {
            let (mut writer, reader) = create_buffer::<u32, 4>();
            let t1 = loom::thread::spawn(move || {
                writer.write(42);
            });
            let t2 = loom::thread::spawn(move || {
                if let Some(v) = reader.try_read(0) {
                    assert_eq!(v, 42);
                }
            });
            t1.join().unwrap();
            t2.join().unwrap();
        });
    }

    #[test]
    fn try_read_consistency() {
        loom::model(|| {
            let (mut writer, reader) = create_buffer::<u32, 4>();

            let t = loom::thread::spawn(move || {
                writer.write(1);
                writer.write(2);
            });

            let t1 = loom::thread::spawn(move || {
                if let Some(v) = reader.try_read(0) {
                    assert!(v == 1 || v == 2);
                }

                if let Some(v) = reader.try_read(1) {
                    assert!(v == 1 || v == 2);
                }
            });

            t.join().unwrap();
            t1.join().unwrap();
        });
    }

    #[test]
    fn try_read_wraparound() {
        loom::model(|| {
            let (mut writer, reader) = create_buffer::<u32, 4>();

            let t = loom::thread::spawn(move || {
                writer.write(1);
                writer.write(2);
                writer.write(3);
                writer.write(4);
                writer.write(5);
            });

            let t1 = loom::thread::spawn(move || {
                if let Some(v) = reader.try_read_latest() {
                    assert!(v <= 5);
                }
            });

            t.join().unwrap();
            t1.join().unwrap();
        });
    }

    #[test]
    fn try_read_multithread() {
        loom::model(|| {
            let (mut writer, reader) = create_buffer::<u32, 4>();

            let t = loom::thread::spawn(move || {
                writer.write(42);
            });

            let r1 = reader.clone();
            let t1 = loom::thread::spawn(move || {
                if let Some(v) = r1.try_read(0) {
                    assert_eq!(v, 42);
                }
            });
            let r2 = reader;
            let t2 = loom::thread::spawn(move || {
                if let Some(v) = r2.try_read(0) {
                    assert_eq!(v, 42);
                }
            });

            t.join().unwrap();
            t1.join().unwrap();
            t2.join().unwrap();
        });
    }

    #[test]
    fn try_read_range() {
        loom::model(|| {
            let (mut writer, reader) = create_buffer::<u32, 4>();

            let t1 = loom::thread::spawn(move || {
                writer.write(1);
                writer.write(2);
                writer.write(3);
            });

            let t2 = loom::thread::spawn(move || {
                let mut buf = Vec::new();
                if reader.try_read_range(.., &mut buf) {
                    assert!(buf.iter().all(|&x| x <= 3 && x >= 1));
                }
            });

            t1.join().unwrap();
            t2.join().unwrap();
        });
    }

    #[test]
    fn read() {
        loom::model(|| {
            let (mut writer, reader) = create_buffer::<u32, 4>();

            let t1 = loom::thread::spawn(move || {
                writer.write(1);
                writer.write(2);
                writer.write(3);
            });

            let t2 = loom::thread::spawn(move || {
                let v1 = reader.read(0).unwrap();
                assert!(v1 <= 3);
                let v2 = reader.read(1).unwrap();
                assert!(v2 <= 2);
                let v3 = reader.read(2).unwrap();
                assert!(v3 <= 1);
            });

            t1.join().unwrap();
            t2.join().unwrap();
        })
    }

    #[test]
    fn read_range() {
        loom::model(|| {
            let (mut writer, reader) = create_buffer::<u32, 4>();
            let t1 = loom::thread::spawn(move || {
                writer.write(1);
                writer.write(2);
                writer.write(3);
            });

            let t2 = loom::thread::spawn(move || {
                let mut buf = Vec::new();
                let _ = reader.read_range(.., &mut buf);
                assert!(buf.iter().all(|&x| x <= 3 && x >= 1));
            });

            t1.join().unwrap();
            t2.join().unwrap();
        });
    }

    #[test]
    fn read_struct() {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        struct Test {
            a: u16,
            b: u16,
            c: u16,
        }
        unsafe impl bytemuck::NoUninit for Test {}

        loom::model(|| {
            let (mut writer, reader) = create_buffer::<Test, 8>();

            let test1 = Test { a: 1, b: 2, c: 3 };
            let test2 = Test { a: 4, b: 5, c: 6 };
            let test3 = Test { a: 7, b: 8, c: 9 };

            let t1 = loom::thread::spawn(move || {
                writer.write(test1);
                writer.write(test2);
                writer.write(test3);
            });

            let t2 = loom::thread::spawn(move || {
                let v1 = reader.read(0).unwrap();
                assert!(v1 == test1 || v1 == test2 || v1 == test3);
                let v2 = reader.read(1).unwrap();
                assert!(v2 == test1 || v2 == test2);
                let v3 = reader.read(2).unwrap();
                assert!(v3 == test1);
            });

            t1.join().unwrap();
            t2.join().unwrap();
        });
    }

    #[test]
    fn read_huge_data() {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        struct BigData {
            big: [u64; 16],
        }
        unsafe impl bytemuck::NoUninit for BigData {}

        loom::model(|| {
            let (mut writer, reader) = create_buffer::<BigData, 4>();

            let t1 = loom::thread::spawn(move || {
                writer.write(BigData { big: [1; 16] });
            });

            let t2 = loom::thread::spawn(move || {
                let v = reader.read_latest();
                assert_eq!(v, BigData { big: [1; 16] });
            });

            t1.join().unwrap();
            t2.join().unwrap();
        });
    }

    #[test]
    fn static_buffer() {
        loom::model(|| {
            let buf = Box::leak(Box::new(StaticBuffer::<u32, 4>::new()));

            let w = &*buf;
            let t1 = loom::thread::spawn(move || {
                let mut writer = w.get_writer().unwrap();
                let _ = writer.write(42);
            });

            let r = &*buf;
            let t2 = loom::thread::spawn(move || {
                if let Some(v) = r.try_read_latest() {
                    assert_eq!(v, 42);
                }
            });

            t1.join().unwrap();
            t2.join().unwrap();
        });
    }

    #[test]
    fn static_buffer_multiple_writes() {
        loom::model(|| {
            let buf = Box::leak(Box::new(StaticBuffer::<u16, 4>::new()));

            let w1 = &*buf;
            let t1 = loom::thread::spawn(move || w1.write(1));

            let w2 = &*buf;
            let t2 = loom::thread::spawn(move || w2.write(2));

            let res1 = t1.join().unwrap();
            let res2 = t2.join().unwrap();

            if res1.is_some() && res2.is_some() {
                let v = buf.try_read(1);
                assert!(v == Some(1) || v == Some(2));
            }
        });
    }
}

#[cfg(test)]
#[cfg(not(feature = "loom"))]
mod tests {
    use super::*;

    #[test]
    fn multiple_try_read() {
        let (mut writer, reader) = create_buffer::<u32, 4>();
        writer.write(42);
        assert_eq!(reader.try_read(0), Some(42));
        assert_eq!(reader.try_read(0), Some(42));
    }

    #[test]
    fn empty() {
        let (_, reader) = create_buffer::<u32, 4>();
        assert_eq!(reader.try_read(0), None);
    }

    #[test]
    fn try_read() {
        let (mut writer, reader) = create_buffer::<u32, 4>();
        writer.write(42);
        assert_eq!(reader.try_read(0), Some(42));
        assert_eq!(reader.try_read(1), None);
    }

    #[test]
    fn try_read_latest() {
        let (mut writer, reader) = create_buffer::<u32, 4>();
        writer.write(42);
        assert_eq!(reader.try_read_latest(), Some(42));
        writer.write(43);
        assert_eq!(reader.try_read_latest(), Some(43));
    }

    #[test]
    fn try_read_range() {
        let (mut writer, reader) = create_buffer::<u32, 4>();
        writer.write(1);
        writer.write(2);
        writer.write(3);
        let mut buf = Vec::new();
        let _ = reader.try_read_range(.., &mut buf);
        assert_eq!(buf, vec![1, 2, 3]);

        let mut buf = Vec::new();
        let _ = reader.try_read_range(1.., &mut buf);
        assert_eq!(buf, vec![1, 2]);
        let mut buf = Vec::new();
        let _ = reader.try_read_range(..2, &mut buf);
        assert_eq!(buf, vec![2, 3]);
    }

    #[test]
    fn try_read_wraparound() {
        let (mut writer, reader) = create_buffer::<u32, 4>();
        writer.write(1);
        writer.write(2);
        writer.write(3);
        writer.write(4);
        writer.write(5);
        assert_eq!(reader.try_read(0), Some(5));
        assert_eq!(reader.try_read(1), Some(4));
    }

    #[test]
    fn try_read_range_wraparound() {
        let (mut writer, reader) = create_buffer::<u32, 4>();
        writer.write(1);
        writer.write(2);
        writer.write(3);
        writer.write(4);
        writer.write(5);
        let mut buf = Vec::new();
        let _ = reader.try_read_range(.., &mut buf);
        assert_eq!(buf, vec![3, 4, 5]);
    }

    #[test]
    fn try_read_range_empty() {
        let (_writer, reader) = create_buffer::<u32, 4>();
        let mut buf = Vec::new();
        let _ = reader.try_read_range(.., &mut buf);
        assert!(buf.is_empty());
    }

    #[test]
    fn read() {
        let (mut writer, reader) = create_buffer::<u32, 4>();
        writer.write(42);
        assert_eq!(reader.read(0), Some(42));
    }

    #[test]
    fn read_latest() {
        let (mut writer, reader) = create_buffer::<u32, 4>();
        writer.write(42);
        assert_eq!(reader.read_latest(), 42);
        writer.write(43);
        assert_eq!(reader.read_latest(), 43);
    }

    #[test]
    fn read_wraparound() {
        let (mut writer, reader) = create_buffer::<u32, 4>();
        writer.write(1);
        writer.write(2);
        writer.write(3);
        writer.write(4);
        writer.write(5);
        assert_eq!(reader.read(0), Some(5));
        assert_eq!(reader.read(1), Some(4));
    }

    #[test]
    fn read_range() {
        let (mut writer, reader) = create_buffer::<u32, 4>();
        writer.write(1);
        writer.write(2);
        writer.write(3);
        let mut buf = Vec::new();
        let _ = reader.read_range(.., &mut buf);
        assert_eq!(buf, vec![1, 2, 3]);

        let mut buf = Vec::new();
        let _ = reader.read_range(1.., &mut buf);
        assert_eq!(buf, vec![1, 2]);
        let mut buf = Vec::new();
        let _ = reader.read_range(..2, &mut buf);
        assert_eq!(buf, vec![2, 3]);
    }

    #[test]
    fn read_range_wraparound() {
        let (mut writer, reader) = create_buffer::<u32, 4>();
        writer.write(1);
        writer.write(2);
        writer.write(3);
        writer.write(4);
        writer.write(5);
        let mut buf = Vec::new();
        let _ = reader.read_range(.., &mut buf);
        assert_eq!(buf, vec![3, 4, 5]);
    }

    #[test]
    fn struct_test() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct Test {
            a: u16,
            b: u16,
            c: u16,
        }
        unsafe impl bytemuck::NoUninit for Test {}

        let (mut writer, reader) = create_buffer::<Test, 8>();
        let test1 = Test { a: 1, b: 2, c: 3 };
        let test2 = Test { a: 4, b: 5, c: 6 };
        let test3 = Test { a: 7, b: 8, c: 9 };
        writer.write(test1);
        writer.write(test2);
        writer.write(test3);
        let val3 = reader.read(0).unwrap();
        let val2 = reader.read(1).unwrap();
        let val1 = reader.read(2).unwrap();
        assert_eq!(val3, test3);
        assert_eq!(val2, test2);
        assert_eq!(val1, test1);
        let mut buf = vec![];
        let _ = reader.read_range(0..3, &mut buf);
        assert_eq!(buf, vec![test1, test2, test3]);
    }

    #[test]
    fn read_huge_data() {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        struct BigData {
            big: [u64; 16],
        }
        unsafe impl bytemuck::NoUninit for BigData {}

        let (mut writer, reader) = create_buffer::<BigData, 4>();
        let data = BigData { big: [1; 16] };
        writer.write(data);
        let val = reader.read(0).unwrap();
        assert_eq!(val, data);
    }

    #[test]
    fn static_buffer() {
        let buf = StaticBuffer::<u32, 4>::new();
        assert!(buf.is_empty());
        let mut writer = buf.get_writer().unwrap();
        writer.write(42);
        assert_eq!(buf.try_read_latest(), Some(42));
    }

    #[test]
    fn static_buffer_multiple_writes() {
        let buf = StaticBuffer::<u32, 4>::new();
        let _ = buf.write(1);
        let _ = buf.write(2);
        let _ = buf.write(3);
        assert_eq!(buf.try_read_latest(), Some(3));
        let mut range = Vec::new();
        let _ = buf.try_read_range(.., &mut range);
        assert_eq!(range, vec![1, 2, 3]);
    }

    #[test]
    fn writer_read() {
        let (mut writer, _reader) = create_buffer::<u32, 4>();
        writer.write(42);
        assert_eq!(writer.read(0), Some(42));
        writer.write(43);
        writer.write(44);
        writer.write(45);
        assert_eq!(writer.read(3), Some(42));
    }

    #[test]
    fn writer_read_range() {
        let (mut writer, _reader) = create_buffer::<u32, 4>();
        writer.write(1);
        writer.write(2);
        writer.write(3);
        let mut buf = Vec::new();
        let _ = writer.read_range(.., &mut buf);
        assert_eq!(buf, vec![1, 2, 3]);
        writer.write(4);
        writer.write(5);
        let mut buf = Vec::new();
        let _ = writer.read_range(.., &mut buf);
        assert_eq!(buf, vec![2, 3, 4, 5]);
    }

    #[cfg(feature = "arrayvec")]
    #[test]
    fn get_range_arrayvec() {
        let (mut writer, reader) = create_buffer::<u32, 4>();
        writer.write(1);
        writer.write(2);
        writer.write(3);
        let mut buf = arrayvec::ArrayVec::<u32, 3>::new();
        let _ = reader.try_read_range(.., &mut buf);
        assert_eq!(buf, arrayvec::ArrayVec::<u32, 3>::from([1, 2, 3]));
    }

    #[cfg(feature = "heapless")]
    #[test]
    fn get_range_heapless() {
        let (mut writer, reader) = create_buffer::<u32, 4>();
        writer.write(1);
        writer.write(2);
        writer.write(3);
        let mut buf = heapless::Vec::<u32, 3>::new();
        let _ = reader.try_read_range(.., &mut buf);
        assert_eq!(buf, heapless::Vec::<u32, 3>::from([1, 2, 3]));
    }
}
