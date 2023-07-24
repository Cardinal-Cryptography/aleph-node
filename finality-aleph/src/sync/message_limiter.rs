use parity_scale_codec::Encode;

use crate::sync::data::MAX_SYNC_MESSAGE_SIZE;

pub const MSG_BYTES_LIMIT: usize = MAX_SYNC_MESSAGE_SIZE as usize;

pub struct Limiter<'a, D: Encode, const LIMIT: usize> {
    msg: &'a [D],
    start_index: usize,
    indexes: Vec<usize>,
}

pub type MsgLimiter<'a, D> = Limiter<'a, D, MSG_BYTES_LIMIT>;

impl<'a, D: Encode, const LIMIT: usize> Limiter<'a, D, LIMIT> {
    pub fn new(msg: &'a [D]) -> Self {
        Self {
            msg,
            start_index: 0,
            indexes: (0..=msg.len()).into_iter().collect(),
        }
    }

    fn next_largest_msg(&mut self) -> Option<&'a [D]> {
        if self.start_index == self.msg.len() {
            return None;
        }
        let end_idx = self.bs_by_encode()?;

        let start_index = self.start_index;
        self.start_index = end_idx;

        Some(&self.msg[start_index..end_idx])
    }

    fn bs_by_encode(&self) -> Option<usize> {
        let indexes = &self.indexes[self.start_index..];

        let idx = indexes.partition_point(|&idx| {
            let encoded_size = self.msg[self.start_index..idx].encoded_size();
            encoded_size <= LIMIT
        }) - 1; // minus 1 since this is the first index where encoded size is larger than LIMIT

        let idx = idx + self.start_index;

        if idx == self.start_index {
            None
        } else {
            Some(idx)
        }
    }
}

impl<'a, D: Encode, const LIMIT: usize> Iterator for Limiter<'a, D, LIMIT> {
    type Item = &'a [D];

    fn next(&mut self) -> Option<Self::Item> {
        self.next_largest_msg()
    }
}

#[cfg(test)]
mod tests {
    use parity_scale_codec::Encode;

    use crate::sync::message_limiter::Limiter;

    type TestLimiter<'a, D> = Limiter<'a, D, 10>;

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct EncodeToSize(usize);

    impl Encode for EncodeToSize {
        fn size_hint(&self) -> usize {
            self.0
        }
        fn encode(&self) -> Vec<u8> {
            vec![0; self.0]
        }
    }

    fn sized(size: usize) -> EncodeToSize {
        EncodeToSize(size)
    }

    #[test]
    fn xxx() {
        let v = vec![sized(5), sized(6), sized(7)];

        let mut lim = TestLimiter::new(&v);

        assert_eq!(Some(&v[..1]), lim.next_largest_msg())
    }
    #[test]
    fn xxx1() {
        let v = vec![sized(1), sized(2), sized(3)];

        let mut lim = TestLimiter::new(&v);

        assert_eq!(Some(&v[..]), lim.next_largest_msg())
    }
    #[test]
    fn xxx2() {
        let v = vec![sized(1), sized(2), sized(7)];

        let mut lim = TestLimiter::new(&v);

        assert_eq!(Some(&v[..2]), lim.next_largest_msg())
    }
    #[test]
    fn xxx3() {
        let v = vec![];

        let mut lim = TestLimiter::<EncodeToSize>::new(&v);

        assert_eq!(None, lim.next_largest_msg())
    }
    #[test]
    fn xxx4() {
        let v = vec![sized(10)];

        let mut lim = TestLimiter::new(&v);

        assert_eq!(None, lim.next_largest_msg())
    }

    #[test]
    fn xxx5() {
        let v = vec![sized(5), sized(6), sized(7)];

        let mut lim = TestLimiter::new(&v);

        assert_eq!(Some(&v[..1]), lim.next());
        assert_eq!(Some(&v[1..2]), lim.next());
        assert_eq!(Some(&v[2..3]), lim.next());
        assert_eq!(None, lim.next());
    }

    #[test]
    fn xxx6() {
        let v = vec![
            sized(5),
            sized(3),
            sized(2),
            sized(5),
            sized(5),
            sized(6),
            sized(7),
        ];

        let mut lim = TestLimiter::new(&v);

        assert_eq!(Some(&v[..2]), lim.next());
        assert_eq!(Some(&v[2..4]), lim.next());
        assert_eq!(Some(&v[4..5]), lim.next());
        assert_eq!(Some(&v[5..6]), lim.next());
        assert_eq!(Some(&v[6..7]), lim.next());
        assert_eq!(None, lim.next());
    }
}
