//! Incremental Annex-B NAL framing, including start codes split across USB reads.
#[derive(Default)]
pub struct AnnexB {
    pending: Vec<u8>,
    scan: usize,
}
impl AnnexB {
    pub fn feed(&mut self, data: &[u8]) -> Result<Vec<Vec<u8>>, &'static str> {
        self.pending.extend_from_slice(data);
        let mut units = Vec::new();
        let mut start = 0;
        let mut found = self.pending.starts_with(&[0, 0, 1]);
        let mut scan = self.scan.max(if found { 3 } else { 0 });
        while scan + 3 <= self.pending.len() {
            if self.pending[scan..scan + 3] == [0, 0, 1] {
                if found && scan > start + 3 {
                    if scan - start > 2_000_000 {
                        self.pending.clear();
                        self.scan = 0;
                        return Err("H.264 NAL exceeds 2 MB; waiting for fresh SPS");
                    }
                    units.push(self.pending[start..scan].to_vec());
                }
                start = scan;
                found = true;
                scan += 3;
            } else {
                scan += 1;
            }
        }
        if found {
            self.pending.drain(..start);
            self.scan = scan - start;
        } else {
            // Retain only a possible incomplete start code, not arbitrary junk.
            let keep = self.pending.len().min(2);
            self.pending.drain(..self.pending.len() - keep);
            self.scan = 0;
        }
        if self.pending.len() > 2_000_000 {
            self.pending.clear();
            self.scan = 0;
            return Err("H.264 NAL exceeds 2 MB; waiting for fresh SPS");
        }
        Ok(units)
    }
}

/// Repeated SPSs are normal. Only changed codec parameters need a new decoder.
#[derive(Default)]
pub struct Parameters(Option<Vec<u8>>);
impl Parameters {
    pub fn changed(&mut self, nal: &[u8]) -> bool {
        if nal.len() < 4 || nal[3] & 31 != 7 {
            return false;
        }
        let mut sps = &nal[3..];
        while sps.last() == Some(&0) {
            sps = &sps[..sps.len() - 1];
        }
        let changed = self.0.as_ref().is_some_and(|old| old != sps);
        self.0 = Some(sps.to_vec());
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_usb_split_preserves_nals_and_emulation_prevention() {
        let bytes = [
            0, 0, 1, 0x67, 100, 0, 52, 0x80, 0, 0, 0, 1, 0x68, 0x80, 0, 0, 1, 0x65, 0, 0, 3, 1,
            0x80, 0, 0, 1, 9, 0x10,
        ];
        let expected = vec![
            bytes[0..9].to_vec(),
            bytes[9..14].to_vec(),
            bytes[14..23].to_vec(),
        ];
        for size in 1..=bytes.len() {
            let mut parser = AnnexB::default();
            let actual: Vec<_> = bytes
                .chunks(size)
                .flat_map(|c| parser.feed(c).unwrap())
                .collect();
            assert_eq!(actual, expected, "USB chunk size {size}");
        }
    }
    #[test]
    fn only_changed_sps_restarts_decoder() {
        let mut parameters = Parameters::default();
        let sps = [0, 0, 1, 0x67, 100, 0, 52, 0x80];
        assert!(!parameters.changed(&sps));
        assert!(!parameters.changed(&[0, 0, 1, 0x68, 0x80]));
        assert!(!parameters.changed(&[sps.as_slice(), &[0]].concat()));
        let mut changed = sps;
        changed[6] = 42;
        assert!(parameters.changed(&changed));
        assert!(!parameters.changed(&changed));
    }
    #[test]
    fn garbage_and_oversize_input_are_bounded() {
        let mut parser = AnnexB::default();
        assert!(parser.feed(&[8; 1000]).unwrap().is_empty());
        assert!(parser.pending.len() <= 2);
        parser.feed(&[0, 0, 1, 0x65]).unwrap();
        assert!(parser.feed(&vec![1; 2_000_001]).is_err());
        assert!(parser.pending.is_empty());
        let mut complete = vec![0, 0, 1, 0x65];
        complete.extend(vec![1; 2_000_001]);
        complete.extend([0, 0, 1, 0x67]);
        assert!(parser.feed(&complete).is_err());
        assert!(parser.pending.is_empty());
    }
}
