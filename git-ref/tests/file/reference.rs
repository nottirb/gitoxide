mod reflog {
    use crate::file;

    #[test]
    fn iter() -> crate::Result {
        let store = file::store()?;
        let r = store.loose_find_existing("HEAD")?;
        let mut buf = Vec::new();
        assert_eq!(r.log_iter(&mut buf)?.expect("log exists").count(), 1);
        Ok(())
    }

    #[test]
    fn iter_rev() -> crate::Result {
        let store = file::store()?;
        let r = store.loose_find_existing("HEAD")?;
        let mut buf = [0u8; 256];
        assert_eq!(r.log_iter_rev(&mut buf)?.expect("log exists").count(), 1);
        Ok(())
    }
}

mod peel {
    use crate::{file, file::store_with_packed_refs};
    use git_testtools::hex_to_id;
    use std::convert::TryFrom;

    #[test]
    fn one_level() -> crate::Result {
        let store = file::store()?;
        let r = store.loose_find_existing("HEAD")?;
        assert_eq!(r.kind(), git_ref::Kind::Symbolic, "there is something to peel");

        let nr =
            git_ref::file::loose::Reference::try_from(r.peel_one_level(None).expect("exists").expect("no failure"))
                .expect("loose ref");
        assert!(
            matches!(nr.target.borrow(), git_ref::Target::Peeled(_)),
            "iteration peels a single level"
        );
        assert!(nr.peel_one_level(None).is_none(), "end of iteration");
        assert_eq!(
            nr.target.borrow(),
            git_ref::Target::Peeled(&hex_to_id("134385f6d781b7e97062102c6a483440bfda2a03")),
            "we still have the peeled target"
        );
        Ok(())
    }

    #[test]
    fn peel_with_packed_involvement() -> crate::Result {
        let store = store_with_packed_refs()?;
        let mut head = store.loose_find_existing("HEAD")?;
        let packed = store.packed()?;
        let expected = hex_to_id("134385f6d781b7e97062102c6a483440bfda2a03");
        assert_eq!(head.peel_to_id_in_place(packed.as_ref())?, expected);
        assert_eq!(head.target.as_id().map(ToOwned::to_owned), Some(expected));

        let mut head = store.find_existing("dt1", packed.as_ref())?;
        assert_eq!(head.peel_to_id_in_place(packed.as_ref())?, expected);
        assert_eq!(head.target().as_id().map(ToOwned::to_owned), Some(expected));
        Ok(())
    }

    #[test]
    fn peel_one_level_with_pack() -> crate::Result {
        let store = store_with_packed_refs()?;
        let packed = store.packed()?;

        let head = store.find_existing("dt1", packed.as_ref())?;
        assert!(head.is_packed());
        assert_eq!(
            head.target().as_id().map(ToOwned::to_owned),
            Some(hex_to_id("4c3f4cce493d7beb45012e478021b5f65295e5a3"))
        );
        assert_eq!(
            head.kind(),
            git_ref::Kind::Peeled,
            "its peeled, but does have another step to peel to"
        );

        let peeled = head
            .peel_one_level(packed.as_ref())
            .expect("a peeled ref for the object")?;
        assert_eq!(
            peeled.target().as_id().map(ToOwned::to_owned),
            Some(hex_to_id("134385f6d781b7e97062102c6a483440bfda2a03")),
            "packed refs are always peeled (at least the ones we choose to read)"
        );
        assert_eq!(peeled.kind(), git_ref::Kind::Peeled, "it's terminally peeled now");
        assert!(peeled.peel_one_level(packed.as_ref()).is_none());
        Ok(())
    }

    #[test]
    fn to_id_multi_hop() -> crate::Result {
        let store = file::store()?;
        let mut r = store.loose_find_existing("multi-link")?;
        assert_eq!(r.kind(), git_ref::Kind::Symbolic, "there is something to peel");

        assert_eq!(
            r.peel_to_id_in_place(None)?,
            hex_to_id("134385f6d781b7e97062102c6a483440bfda2a03")
        );
        assert_eq!(r.name.as_bstr(), "refs/remotes/origin/multi-link-target3");

        Ok(())
    }

    #[test]
    fn to_id_cycle() -> crate::Result {
        let store = file::store()?;
        let mut r = store.loose_find_existing("loop-a")?;
        assert_eq!(r.kind(), git_ref::Kind::Symbolic, "there is something to peel");
        assert_eq!(r.name.as_bstr(), "refs/loop-a");

        assert!(matches!(
            r.peel_to_id_in_place(None).unwrap_err(),
            git_ref::file::loose::reference::peel::to_id::Error::Cycle { .. }
        ));
        assert_eq!(r.name.as_bstr(), "refs/loop-a", "the ref is not changed on error");
        Ok(())
    }
}

mod parse {
    use git_ref::file::Store;

    fn store() -> Store {
        Store::at("base doesnt matter", Default::default())
    }

    mod invalid {
        use crate::file::reference::parse::store;
        use git_ref::file::loose::Reference;

        macro_rules! mktest {
            ($name:ident, $input:literal, $err:literal) => {
                #[test]
                fn $name() {
                    use std::convert::TryInto;
                    let store = store();
                    let err = Reference::try_from_path(
                        &store,
                        "HEAD".try_into().expect("this is a valid name"),
                        $input,
                    )
                    .unwrap_err();
                    assert_eq!(err.to_string(), $err);
                }
            };
        }

        mktest!(hex_id, b"foobar", "\"foobar\" could not be parsed");
        mktest!(ref_tag, b"reff: hello", "\"reff: hello\" could not be parsed");
    }
    mod valid {
        use crate::file::reference::parse::store;
        use bstr::ByteSlice;
        use git_ref::file::loose::Reference;
        use git_testtools::hex_to_id;

        macro_rules! mktest {
            ($name:ident, $input:literal, $kind:path, $id:expr, $ref:expr) => {
                #[test]
                fn $name() {
                    use std::convert::TryInto;
                    let store = store();
                    let reference = Reference::try_from_path(
                        &store,
                        "HEAD".try_into().expect("valid static name"),
                        $input,
                    )
                    .unwrap();
                    assert_eq!(reference.kind(), $kind);
                    assert_eq!(reference.target.borrow().as_id(), $id);
                    assert_eq!(reference.target.borrow().as_name(), $ref);
                }
            };
        }

        mktest!(
            peeled,
            b"c5241b835b93af497cda80ce0dceb8f49800df1c\n",
            git_ref::Kind::Peeled,
            Some(hex_to_id("c5241b835b93af497cda80ce0dceb8f49800df1c").as_ref()),
            None
        );

        mktest!(
            symbolic,
            b"ref: refs/heads/main\n",
            git_ref::Kind::Symbolic,
            None,
            Some(b"refs/heads/main".as_bstr())
        );

        mktest!(
            symbolic_more_than_one_space,
            b"ref:        refs/foobar\n",
            git_ref::Kind::Symbolic,
            None,
            Some(b"refs/foobar".as_bstr())
        );
    }
}
