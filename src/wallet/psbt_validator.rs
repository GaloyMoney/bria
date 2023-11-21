use bdk::bitcoin::psbt;

use std::collections::HashSet;

use crate::xpub::XPub;

use super::error::WalletError;

pub fn validate_psbt(
    signed_psbt: &psbt::PartiallySignedTransaction,
    xpub: XPub,
    unsigned_psbt: &psbt::PartiallySignedTransaction,
) -> Result<(), WalletError> {
    let set: HashSet<_> = signed_psbt
        .inputs
        .iter()
        .flat_map(|inp| &inp.bip32_derivation)
        .filter_map(|(pk, (fingerprint, _))| {
            (fingerprint == &xpub.inner().parent_fingerprint).then_some(pk)
        })
        .collect();

    if unsigned_psbt.unsigned_tx != signed_psbt.unsigned_tx {
        return Err(WalletError::UnsignedTxnMismatch);
    }

    if !(signed_psbt
        .inputs
        .iter()
        .all(|inp| inp.final_script_witness.is_some() || inp.final_script_sig.is_some())
        || signed_psbt
            .inputs
            .iter()
            .flat_map(|inp| &inp.partial_sigs)
            .any(|(pk, _)| set.contains(&pk.inner)))
    {
        return Err(WalletError::PsbtDoesNotHaveValidSignatures);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_if_correct_and_signed_psbt_passed() {
        let xpub = XPub::try_from(("tpubDE8HT914zGpxhJhgoMX35xgNyjHy5d1neGXHjTLAtuUssTA7tNWNs177JsFPbJwD5FBXCHJYbwUC9AzSEpYHC4hKgaCvZyZTuCbWfNUWXoM", Some("m/48h/1h/0h/2h"))).unwrap();
        let signed_psbt = "cHNidP8BAH0BAAAAASNihqnLFfz7pHt1zDeB/iB7ku75Ah6EFaFhQZnbErt9AAAAAAD+////Ap13fQEAAAAAIgAgO37beKyitaViJwyjZ3oTIwdBU0JTbBRa32V1zvdifQzAaHgEAAAAABYAFFPOvhKDbGzCHM0LNEHgSPJjuf7RzQAAAAABAIkCAAAAAUxGIfiVmAY20gYMGPkMWDhNuf7xZOc3UutYyrXNRyY2AAAAAAD9////AgDh9QUAAAAAIgAgjoqVjwo7KNRKWUHLalHhejeEI0zUN3PteWscxop8ElYcBBAkAQAAACJRIM5ovh2uzu6dPIaxxMy66uvDCZUg1uNFd/ZG6kvgK5kzyAAAAAEBKwDh9QUAAAAAIgAgjoqVjwo7KNRKWUHLalHhejeEI0zUN3PteWscxop8ElYBCJIDAEcwRAIgGZdgjGq/M/51nE9WtP69BZBhQtho22JcoIQHSWEXI00CIA7/Mj5A906MFjd+sm+EawhjTALyR5jsPyT6Qa7TEJQDAUdRIQJQZ+FcB64peA2v9qxsxfxWZzNIJwIuTOwO4hzMTAOLSCECl48Dr84329WNBzLx9gXNhKrbpMfncXeFfKjrrNt6hoZSrgABAUdRIQJcOcC8y4Cq1oHFG9ZZErhw54kKafGsebNftiz5M6AuQyECyXb40F/RY1cHkPK7+PT6W4hVgJX7bUZHuE8jYBS+KyJSriICAlw5wLzLgKrWgcUb1lkSuHDniQpp8ax5s1+2LPkzoC5DHJhT3akwAACAAQAAgAAAAIACAACAAQAAAAAAAAAiAgLJdvjQX9FjVweQ8rv49PpbiFWAlfttRke4TyNgFL4rIhwd6KQcMAAAgAEAAIAAAACAAgAAgAEAAAAAAAAAAAA=".parse::<psbt::PartiallySignedTransaction>().unwrap();
        let unsigned_psbt = "cHNidP8BAH0BAAAAASNihqnLFfz7pHt1zDeB/iB7ku75Ah6EFaFhQZnbErt9AAAAAAD+////Ap13fQEAAAAAIgAgO37beKyitaViJwyjZ3oTIwdBU0JTbBRa32V1zvdifQzAaHgEAAAAABYAFFPOvhKDbGzCHM0LNEHgSPJjuf7RzQAAAAABAPYCAAAAAAEBTEYh+JWYBjbSBgwY+QxYOE25/vFk5zdS61jKtc1HJjYAAAAAAP3///8CAOH1BQAAAAAiACCOipWPCjso1EpZQctqUeF6N4QjTNQ3c+15axzGinwSVhwEECQBAAAAIlEgzmi+Ha7O7p08hrHEzLrq68MJlSDW40V39kbqS+ArmTMCRzBEAiB5fcQ8lx7fp+Calgy7o9jQEsHEPho0zfP13TQsCC2/GgIgSL/zyp0nz5PzdMXxhgBJ59O2t7tUhAfKxBYtVjMYXR0BIQN39pz1kuRtgfVu5SMba1rXL5HXDIKq4/rq7I/342+/GsgAAAABASsA4fUFAAAAACIAII6KlY8KOyjUSllBy2pR4Xo3hCNM1Ddz7XlrHMaKfBJWAQMEAQAAAAEFR1EhAlBn4VwHril4Da/2rGzF/FZnM0gnAi5M7A7iHMxMA4tIIQKXjwOvzjfb1Y0HMvH2Bc2Eqtukx+dxd4V8qOus23qGhlKuIgYCUGfhXAeuKXgNr/asbMX8VmczSCcCLkzsDuIczEwDi0gcmFPdqTAAAIABAACAAAAAgAIAAIAAAAAAAAAAACIGApePA6/ON9vVjQcy8fYFzYSq26TH53F3hXyo66zbeoaGHB3opBwwAACAAQAAgAAAAIACAACAAAAAAAAAAAAAAQFHUSECXDnAvMuAqtaBxRvWWRK4cOeJCmnxrHmzX7Ys+TOgLkMhAsl2+NBf0WNXB5Dyu/j0+luIVYCV+21GR7hPI2AUvisiUq4iAgJcOcC8y4Cq1oHFG9ZZErhw54kKafGsebNftiz5M6AuQxyYU92pMAAAgAEAAIAAAACAAgAAgAEAAAAAAAAAIgICyXb40F/RY1cHkPK7+PT6W4hVgJX7bUZHuE8jYBS+KyIcHeikHDAAAIABAACAAAAAgAIAAIABAAAAAAAAAAAA".parse::<psbt::PartiallySignedTransaction>().unwrap();
        assert_eq!(
            validate_psbt(&signed_psbt, xpub, &unsigned_psbt).unwrap(),
            ()
        );
    }

    #[test]
    fn fails_if_incorrect_and_signed_psbt_passed() {
        let xpub = XPub::try_from(("tpubDE8HT914zGpxhJhgoMX35xgNyjHy5d1neGXHjTLAtuUssTA7tNWNs177JsFPbJwD5FBXCHJYbwUC9AzSEpYHC4hKgaCvZyZTuCbWfNUWXoM", Some("m/48h/1h/0h/2h"))).unwrap();
        let signed_psbt = "cHNidP8BAHEBAAAAAYbn7MxiehS1ZSM0EB5cRE9nSFCZmsSG1esfWwyuVz+GAAAAAAD+////ArN3fQEAAAAAFgAU/gKMRui+SfQrLooISrMz67mo2TDAaHgEAAAAABYAFFPOvhKDbGzCHM0LNEHgSPJjuf7RzQAAAAABAHECAAAAAUHSbz5JON/N5LECN+D4fkQ4ePau7Swxe/W6Abiyik7/AAAAAAD9////AgDh9QUAAAAAFgAUmtl/mRY8rv7U7qlTERct+67KKEL8BRAkAQAAABYAFNwYhIQ2+xR1XyFfuB0O8PfBJB0ayAAAAAEBHwDh9QUAAAAAFgAUmtl/mRY8rv7U7qlTERct+67KKEIBCGsCRzBEAiAbOAYSbHJagpdcvez9jSFo4tWsN/cFBi1RtRHwh11t/gIgZbnkC8tKwAUMrF+dGjI6w1iChJHwN+Cs2VVhqCuEyWoBIQOZcT3zA/OKNzoMwjrfDPXErOVu7/Tevy9zOUkioZCXbwAiAgNZOo5LB+cTuY4cZu9Oa+w14tqxpMhUdA8YDniI9uRBKhhvL6GyVAAAgAAAAIAAAACAAQAAAAAAAAAAAA==".parse::<psbt::PartiallySignedTransaction>().unwrap();
        let unsigned_psbt = "cHNidP8BAH0BAAAAASNihqnLFfz7pHt1zDeB/iB7ku75Ah6EFaFhQZnbErt9AAAAAAD+////Ap13fQEAAAAAIgAgO37beKyitaViJwyjZ3oTIwdBU0JTbBRa32V1zvdifQzAaHgEAAAAABYAFFPOvhKDbGzCHM0LNEHgSPJjuf7RzQAAAAABAPYCAAAAAAEBTEYh+JWYBjbSBgwY+QxYOE25/vFk5zdS61jKtc1HJjYAAAAAAP3///8CAOH1BQAAAAAiACCOipWPCjso1EpZQctqUeF6N4QjTNQ3c+15axzGinwSVhwEECQBAAAAIlEgzmi+Ha7O7p08hrHEzLrq68MJlSDW40V39kbqS+ArmTMCRzBEAiB5fcQ8lx7fp+Calgy7o9jQEsHEPho0zfP13TQsCC2/GgIgSL/zyp0nz5PzdMXxhgBJ59O2t7tUhAfKxBYtVjMYXR0BIQN39pz1kuRtgfVu5SMba1rXL5HXDIKq4/rq7I/342+/GsgAAAABASsA4fUFAAAAACIAII6KlY8KOyjUSllBy2pR4Xo3hCNM1Ddz7XlrHMaKfBJWAQMEAQAAAAEFR1EhAlBn4VwHril4Da/2rGzF/FZnM0gnAi5M7A7iHMxMA4tIIQKXjwOvzjfb1Y0HMvH2Bc2Eqtukx+dxd4V8qOus23qGhlKuIgYCUGfhXAeuKXgNr/asbMX8VmczSCcCLkzsDuIczEwDi0gcmFPdqTAAAIABAACAAAAAgAIAAIAAAAAAAAAAACIGApePA6/ON9vVjQcy8fYFzYSq26TH53F3hXyo66zbeoaGHB3opBwwAACAAQAAgAAAAIACAACAAAAAAAAAAAAAAQFHUSECXDnAvMuAqtaBxRvWWRK4cOeJCmnxrHmzX7Ys+TOgLkMhAsl2+NBf0WNXB5Dyu/j0+luIVYCV+21GR7hPI2AUvisiUq4iAgJcOcC8y4Cq1oHFG9ZZErhw54kKafGsebNftiz5M6AuQxyYU92pMAAAgAEAAIAAAACAAgAAgAEAAAAAAAAAIgICyXb40F/RY1cHkPK7+PT6W4hVgJX7bUZHuE8jYBS+KyIcHeikHDAAAIABAACAAAAAgAIAAIABAAAAAAAAAAAA".parse::<psbt::PartiallySignedTransaction>().unwrap();
        assert!(matches!(
            validate_psbt(&signed_psbt, xpub, &unsigned_psbt),
            Err(WalletError::UnsignedTxnMismatch)
        ));
    }

    #[test]
    fn fails_if_unsigned_psbt_passed() {
        let xpub = XPub::try_from(("tpubDE8HT914zGpxhJhgoMX35xgNyjHy5d1neGXHjTLAtuUssTA7tNWNs177JsFPbJwD5FBXCHJYbwUC9AzSEpYHC4hKgaCvZyZTuCbWfNUWXoM", Some("m/48h/1h/0h/2h"))).unwrap();
        let signed_psbt = "cHNidP8BAH0BAAAAASNihqnLFfz7pHt1zDeB/iB7ku75Ah6EFaFhQZnbErt9AAAAAAD+////Ap13fQEAAAAAIgAgO37beKyitaViJwyjZ3oTIwdBU0JTbBRa32V1zvdifQzAaHgEAAAAABYAFFPOvhKDbGzCHM0LNEHgSPJjuf7RzQAAAAABAPYCAAAAAAEBTEYh+JWYBjbSBgwY+QxYOE25/vFk5zdS61jKtc1HJjYAAAAAAP3///8CAOH1BQAAAAAiACCOipWPCjso1EpZQctqUeF6N4QjTNQ3c+15axzGinwSVhwEECQBAAAAIlEgzmi+Ha7O7p08hrHEzLrq68MJlSDW40V39kbqS+ArmTMCRzBEAiB5fcQ8lx7fp+Calgy7o9jQEsHEPho0zfP13TQsCC2/GgIgSL/zyp0nz5PzdMXxhgBJ59O2t7tUhAfKxBYtVjMYXR0BIQN39pz1kuRtgfVu5SMba1rXL5HXDIKq4/rq7I/342+/GsgAAAABASsA4fUFAAAAACIAII6KlY8KOyjUSllBy2pR4Xo3hCNM1Ddz7XlrHMaKfBJWAQMEAQAAAAEFR1EhAlBn4VwHril4Da/2rGzF/FZnM0gnAi5M7A7iHMxMA4tIIQKXjwOvzjfb1Y0HMvH2Bc2Eqtukx+dxd4V8qOus23qGhlKuIgYCUGfhXAeuKXgNr/asbMX8VmczSCcCLkzsDuIczEwDi0gcmFPdqTAAAIABAACAAAAAgAIAAIAAAAAAAAAAACIGApePA6/ON9vVjQcy8fYFzYSq26TH53F3hXyo66zbeoaGHB3opBwwAACAAQAAgAAAAIACAACAAAAAAAAAAAAAAQFHUSECXDnAvMuAqtaBxRvWWRK4cOeJCmnxrHmzX7Ys+TOgLkMhAsl2+NBf0WNXB5Dyu/j0+luIVYCV+21GR7hPI2AUvisiUq4iAgJcOcC8y4Cq1oHFG9ZZErhw54kKafGsebNftiz5M6AuQxyYU92pMAAAgAEAAIAAAACAAgAAgAEAAAAAAAAAIgICyXb40F/RY1cHkPK7+PT6W4hVgJX7bUZHuE8jYBS+KyIcHeikHDAAAIABAACAAAAAgAIAAIABAAAAAAAAAAAA".parse::<psbt::PartiallySignedTransaction>().unwrap();
        let unsigned_psbt = "cHNidP8BAH0BAAAAASNihqnLFfz7pHt1zDeB/iB7ku75Ah6EFaFhQZnbErt9AAAAAAD+////Ap13fQEAAAAAIgAgO37beKyitaViJwyjZ3oTIwdBU0JTbBRa32V1zvdifQzAaHgEAAAAABYAFFPOvhKDbGzCHM0LNEHgSPJjuf7RzQAAAAABAPYCAAAAAAEBTEYh+JWYBjbSBgwY+QxYOE25/vFk5zdS61jKtc1HJjYAAAAAAP3///8CAOH1BQAAAAAiACCOipWPCjso1EpZQctqUeF6N4QjTNQ3c+15axzGinwSVhwEECQBAAAAIlEgzmi+Ha7O7p08hrHEzLrq68MJlSDW40V39kbqS+ArmTMCRzBEAiB5fcQ8lx7fp+Calgy7o9jQEsHEPho0zfP13TQsCC2/GgIgSL/zyp0nz5PzdMXxhgBJ59O2t7tUhAfKxBYtVjMYXR0BIQN39pz1kuRtgfVu5SMba1rXL5HXDIKq4/rq7I/342+/GsgAAAABASsA4fUFAAAAACIAII6KlY8KOyjUSllBy2pR4Xo3hCNM1Ddz7XlrHMaKfBJWAQMEAQAAAAEFR1EhAlBn4VwHril4Da/2rGzF/FZnM0gnAi5M7A7iHMxMA4tIIQKXjwOvzjfb1Y0HMvH2Bc2Eqtukx+dxd4V8qOus23qGhlKuIgYCUGfhXAeuKXgNr/asbMX8VmczSCcCLkzsDuIczEwDi0gcmFPdqTAAAIABAACAAAAAgAIAAIAAAAAAAAAAACIGApePA6/ON9vVjQcy8fYFzYSq26TH53F3hXyo66zbeoaGHB3opBwwAACAAQAAgAAAAIACAACAAAAAAAAAAAAAAQFHUSECXDnAvMuAqtaBxRvWWRK4cOeJCmnxrHmzX7Ys+TOgLkMhAsl2+NBf0WNXB5Dyu/j0+luIVYCV+21GR7hPI2AUvisiUq4iAgJcOcC8y4Cq1oHFG9ZZErhw54kKafGsebNftiz5M6AuQxyYU92pMAAAgAEAAIAAAACAAgAAgAEAAAAAAAAAIgICyXb40F/RY1cHkPK7+PT6W4hVgJX7bUZHuE8jYBS+KyIcHeikHDAAAIABAACAAAAAgAIAAIABAAAAAAAAAAAA".parse::<psbt::PartiallySignedTransaction>().unwrap();
        assert!(matches!(
            validate_psbt(&signed_psbt, xpub, &unsigned_psbt),
            Err(WalletError::PsbtDoesNotHaveValidSignatures)
        ))
    }
}
