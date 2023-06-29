use bdk::bitcoin;

use bria::wallet::psbt_validator;
use bria::xpub::*;

#[tokio::test]
async fn psbt_validation_fails_when_no_signature_found() -> anyhow::Result<()> {
    let xpub = XPub::try_from(("tpubDCr5twyowBZhQZEdAXWeJgZtKZgGbKSY4Co55hgw551xCZtHk5fWw9EyGKDBE6cSZPzc4QWR4NyZAeuZDKvRHpQmch78CKwLSy8FEhbvBeR", Some("m/84h/0h/0h"))).unwrap();
    let unsigned_psbt = "cHNidP8BAHEBAAAAAQXU7rw5wiONrLs8zEBacpUPaeCbbFvUGt+ZGhYXNAeIAQAAAAD+////ArN3fQEAAAAAFgAUV69hkddpchM/Kdbp6TUN+rIcmr7AaHgEAAAAABYAFFPOvhKDbGzCHM0LNEHgSPJjuf7RzQAAAAABAHECAAAAAU9hZA8fbnJG2yVoilG86fTB2OL1IqrvPQA9NlBtWiQnAAAAAAD9////AvwFECQBAAAAFgAUzLEhaay70eCwLvJFdjkvtol3NW8A4fUFAAAAABYAFDBOCTOounYzbvGLZnREjy6LIYleyAAAAAEBHwDh9QUAAAAAFgAUME4JM6i6djNu8YtmdESPLoshiV4BAwQBAAAAIgYD5r7WIWyxhitgzbBviYDuNdIi5MFngT7p77MpjifMr1oYb9vpm1QAAIAAAACAAAAAgAAAAAAAAAAAACICA/rQ0V1BsVPLAcQg2Hztdzw16PuDuhOzci1DnSHfjcpBGG/b6ZtUAACAAAAAgAAAAIABAAAAEAAAAAAA".parse::<bitcoin::psbt::PartiallySignedTransaction>().unwrap();
    assert!(!psbt_validator::validate_psbt(&unsigned_psbt, xpub));
    Ok(())
}

#[tokio::test]
async fn psbt_validation_passes_when_psbt_has_signature_corresponding_to_xpub() -> anyhow::Result<()>
{
    let xpub = XPub::try_from(("tpubDCr5twyowBZhQZEdAXWeJgZtKZgGbKSY4Co55hgw551xCZtHk5fWw9EyGKDBE6cSZPzc4QWR4NyZAeuZDKvRHpQmch78CKwLSy8FEhbvBeR", Some("m/84h/0h/0h"))).unwrap();
    let signed_psbt = "cHNidP8BAHEBAAAAAW4QOT19+Uo9u6t7aDKmPXO/3w/xngnKmx9dTVANte9UAAAAAAD+////AuBwcgAAAAAAFgAUiMb1OtZlnYl675jK0AyiiKL9aDaTb4MFAAAAABYAFCZ1Q/XoJGSYzKxIS4BxneQgcF76zQAAAAABAN4CAAAAAAEBaptFtxfQBPJQn8F4EEXhoFdO4e4iMPviJ1CzgYJfbiIAAAAAAP3///8CAOH1BQAAAAAWABQwTgkzqLp2M27xi2Z0RI8uiyGJXvwFECQBAAAAFgAUTee+ueYwAcnhDxp0DTYDR1ZkGJ8CRzBEAiAvABR181UCFonR47sNobPua6tPN2eaQPeuD1hXlt1hRgIgTTA2C0YDhXKnbyYlBqeNqAxrEFezQ2qkI9ua7vaGf2gBIQKjUJi5wYVZ2EWXNdIeJcVp4wIdRh2+hiBhJHc7ZA9gL8gAAAABAR8A4fUFAAAAABYAFDBOCTOounYzbvGLZnREjy6LIYleIgID5r7WIWyxhitgzbBviYDuNdIi5MFngT7p77MpjifMr1pHMEQCIHoaBu0iBiJ2ZoggAPvO5AXQ+l3WoxRc5x/ppIyNX93lAiA878wEcBTYNS8igFYy/m8RyiN0RR60Y3VNCeiqMxDyewEBAwQBAAAAIgYD5r7WIWyxhitgzbBviYDuNdIi5MFngT7p77MpjifMr1oYb9vpm1QAAIAAAACAAAAAgAAAAAAAAAAAAAAiAgO+qPqnLtz1qY0jb2Xn1qppkA8Fo4g2wjaVQhzw9d2r4hhv2+mbVAAAgAAAAIAAAACAAQAAAAMAAAAA".parse::<bitcoin::psbt::PartiallySignedTransaction>().unwrap();
    assert!(psbt_validator::validate_psbt(&signed_psbt, xpub));
    Ok(())
}

#[tokio::test]
async fn psbt_validation_fails_when_psbt_does_have_signature_corresponding_to_xpub(
) -> anyhow::Result<()> {
    let xpub = XPub::try_from(("tpubDD4vFnWuTMEcZiaaZPgvzeGyMzWe6qHW8gALk5Md9kutDvtdDjYFwzauEFFRHgov8pAwup5jX88j5YFyiACsPf3pqn5hBjvuTLRAseaJ6b4", Some("m/84h/0h/0h"))).unwrap();
    let signed_psbt = "cHNidP8BAHEBAAAAAW4QOT19+Uo9u6t7aDKmPXO/3w/xngnKmx9dTVANte9UAAAAAAD+////AuBwcgAAAAAAFgAUiMb1OtZlnYl675jK0AyiiKL9aDaTb4MFAAAAABYAFCZ1Q/XoJGSYzKxIS4BxneQgcF76zQAAAAABAN4CAAAAAAEBaptFtxfQBPJQn8F4EEXhoFdO4e4iMPviJ1CzgYJfbiIAAAAAAP3///8CAOH1BQAAAAAWABQwTgkzqLp2M27xi2Z0RI8uiyGJXvwFECQBAAAAFgAUTee+ueYwAcnhDxp0DTYDR1ZkGJ8CRzBEAiAvABR181UCFonR47sNobPua6tPN2eaQPeuD1hXlt1hRgIgTTA2C0YDhXKnbyYlBqeNqAxrEFezQ2qkI9ua7vaGf2gBIQKjUJi5wYVZ2EWXNdIeJcVp4wIdRh2+hiBhJHc7ZA9gL8gAAAABAR8A4fUFAAAAABYAFDBOCTOounYzbvGLZnREjy6LIYleIgID5r7WIWyxhitgzbBviYDuNdIi5MFngT7p77MpjifMr1pHMEQCIHoaBu0iBiJ2ZoggAPvO5AXQ+l3WoxRc5x/ppIyNX93lAiA878wEcBTYNS8igFYy/m8RyiN0RR60Y3VNCeiqMxDyewEBAwQBAAAAIgYD5r7WIWyxhitgzbBviYDuNdIi5MFngT7p77MpjifMr1oYb9vpm1QAAIAAAACAAAAAgAAAAAAAAAAAAAAiAgO+qPqnLtz1qY0jb2Xn1qppkA8Fo4g2wjaVQhzw9d2r4hhv2+mbVAAAgAAAAIAAAACAAQAAAAMAAAAA".parse::<bitcoin::psbt::PartiallySignedTransaction>().unwrap();
    assert!(!psbt_validator::validate_psbt(&signed_psbt, xpub));
    Ok(())
}
