//! What a `fonts` request carries: the images and letter sheets your interface is drawn with, and
//! where on a text line each is drawn. Plain data; custom-models checks and builds it.

/// One picture drawn as one character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Image<'a> {
    /// `[a-z0-9_]`, 1 to 32 characters.
    pub name: &'a str,
    /// A whole RGBA PNG, at most 256 high and 512 wide; wider than 256 it is drawn as strips.
    pub png: &'a [u8],
}

/// A grid of letters in one image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sheet<'a> {
    /// `[a-z0-9_]`, 1 to 32 characters.
    pub name: &'a str,
    /// A whole RGBA PNG, at most 256 x 256, cut into equal boxes: as many columns as a row has
    /// characters, as many rows as `rows` has.
    pub png: &'a [u8],
    /// The letter of each box, row by row. A space stands for a box that is not a letter.
    pub rows: Vec<&'a str>,
    /// The advance of a space in this font, in pixels (1 to 32).
    pub space: u8,
}

/// Where something is drawn: `top` is the distance from the top of the text line down to the top
/// of the image or of the sheet's boxes, in pixels (negative: above the line).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Place<'a> {
    Image { image: &'a str, top: i16 },
    Sheet { sheet: &'a str, top: i16 },
}

/// Everything one plugin wants drawn.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Request<'a> {
    pub images: Vec<Image<'a>>,
    pub sheets: Vec<Sheet<'a>>,
    pub places: Vec<Place<'a>>,
}
