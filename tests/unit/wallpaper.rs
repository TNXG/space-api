#[cfg(test)]
mod tests {
    #[test]
    fn random_index_stays_within_bounds() {
        for len in [1usize, 2, 3, 7, 348] {
            for _ in 0..500 {
                assert!(super::random_index(len).expect("os entropy available") < len);
            }
        }
        assert!(super::random_index(0).is_err());
    }

    #[test]
    fn random_index_covers_every_position() {
        const LEN: usize = 7;
        const DRAWS: usize = 10_000;
        let mut seen = [0usize; LEN];
        for _ in 0..DRAWS {
            seen[super::random_index(LEN).expect("os entropy available")] += 1;
        }
        let expected = DRAWS / LEN;
        assert!(
            seen.iter().all(|&c| c > expected - 400 && c < expected + 400),
            "skewed distribution: {seen:?}"
        );
    }


    #[test]
    fn pick_entry_returns_some_record_verbatim() {
        let entries = vec![
            ("a.jpg".to_string(), "hash-a".to_string()),
            ("b23_p0.png".to_string(), "hash-b".to_string()),
            ("c.webp".to_string(), "hash-c".to_string()),
        ];
        let mut seen = std::collections::HashSet::new();
        for _ in 0..2_000 {
            let (filename, hash) = super::pick_entry(&entries).expect("entries are non-empty");
            assert!(entries.contains(&(filename.clone(), hash.clone())));
            seen.insert(filename.clone());
        }
        assert_eq!(seen.len(), entries.len());
        assert!(super::pick_entry(&Vec::new()).is_err());
    }

    #[test]
    fn wallpaper_index_keeps_key_and_blurhash_in_sync() {
        let data = &*super::BLURHASH;
        for (entries, map) in [
            (&*super::WEIGHT_ENTRIES, &data.weight),
            (&*super::HEIGHT_ENTRIES, &data.height),
        ] {
            assert_eq!(entries.len(), map.len());
            for (filename, hash) in entries.iter() {
                assert_eq!(map.get(filename), Some(hash));
                assert!(!filename.is_empty());
            }
        }
    }
}
