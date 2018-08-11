///`Content-Encoding` header
pub enum ContentEncoding {
    ///Indicates that no compression is taken place.
    Identity,
    ///Indicates compression using Gzip.
    Gzip,
    ///Indicates compression using Deflate.
    Deflate,
    ///Indicates compression using Brotli.
    Brotli
}

impl ContentEncoding {
    ///Returns whether encoding indicates compression or not
    pub fn is_compression(&self) -> bool {
        match *self {
            ContentEncoding::Identity => false,
            _ => true,
        }
    }

    ///Returns whether Yukikaze-sama can decompress.
    ///
    ///Based on enabled features.
    ///
    ///Note that if decompression is not possible,
    ///user must decompress himself.
    pub fn can_decompress(&self) -> bool {
        match *self {
            #[cfg(feature = "flate2")]
            ContentEncoding::Gzip => true,
            #[cfg(feature = "flate2")]
            ContentEncoding::Deflate => true,
            _ => false,
        }
    }

    ///Returns textual representation.
    pub fn as_str(&self) -> &'static str {
        match *self {
            ContentEncoding::Identity => "identity",
            ContentEncoding::Gzip => "gzip",
            ContentEncoding::Deflate => "deflate",
            ContentEncoding::Brotli => "br",
        }
    }
}

impl<'a> From<&'a str> for ContentEncoding {
    fn from(text: &'a str) -> ContentEncoding {
        match text {
            "br" => ContentEncoding::Brotli,
            "gzip" => ContentEncoding::Gzip,
            "deflate" => ContentEncoding::Deflate,
            _ => ContentEncoding::Identity,
        }
    }
}
