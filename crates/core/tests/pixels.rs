use wove::{
    elements::{split, Blocks, Pixels},
    testing::{assert_frame, Screen},
    Color, Id,
};

fn show(screen: &mut Screen, pixels: Pixels) -> Id {
    screen.tree.add(screen.tree.root(), pixels).unwrap()
}

#[test]
fn half_blocks_show_two_pixels_per_cell() {
    let mut pixels = Pixels::new(3, 2);
    pixels.set(0, 0, [255, 0, 0]);
    pixels.set(1, 1, [0, 0, 255]);
    pixels.set(2, 0, [0, 255, 0]);
    pixels.set(2, 1, [0, 255, 0]);
    let mut screen = Screen::new(3, 1);
    show(&mut screen, pixels);
    let frame = screen.frame().unwrap();
    // The upper pixel is the glyph's color and the lower one the background,
    // and a cell of one color needs no glyph.
    assert_frame(frame, "▀▀\n@cursor none\n");
    let cell = |x| frame.cell(x, 0).unwrap();
    assert_eq!(cell(0).style().fg, Color::Rgb(255, 0, 0));
    assert_eq!(cell(0).style().bg, Color::Rgb(0, 0, 0));
    assert_eq!(cell(1).style().fg, Color::Rgb(0, 0, 0));
    assert_eq!(cell(1).style().bg, Color::Rgb(0, 0, 255));
    assert_eq!(cell(2).symbol(), " ");
    assert_eq!(cell(2).style().bg, Color::Rgb(0, 255, 0));
}

#[test]
fn quadrants_and_sextants_pack_more_pixels_into_a_cell() {
    let mut quad = Pixels::new(2, 2);
    quad.blocks = Blocks::Quadrant;
    quad.set(0, 0, [255; 3]);
    quad.set(1, 1, [255; 3]);
    let mut screen = Screen::new(1, 1);
    show(&mut screen, quad);
    assert_frame(screen.frame().unwrap(), "▚\n@cursor none\n");

    let mut sextant = Pixels::new(2, 3);
    sextant.blocks = Blocks::Sextant;
    sextant.set(0, 0, [255; 3]);
    sextant.set(1, 2, [255; 3]);
    let mut screen = Screen::new(1, 1);
    show(&mut screen, sextant);
    let frame = screen.frame().unwrap();
    // Pixels one and six: pattern 33, the twenty-ninth sextant after the
    // left half block is skipped.
    assert_eq!(frame.cell(0, 0).unwrap().symbol(), "\u{1FB1F}");
    let mut sextant = Pixels::new(2, 3);
    sextant.blocks = Blocks::Sextant;
    for y in 0..3 {
        sextant.set(0, y, [255; 3]);
    }
    let mut screen = Screen::new(1, 1);
    show(&mut screen, sextant);
    assert_eq!(screen.frame().unwrap().cell(0, 0).unwrap().symbol(), "▌");
}

#[test]
fn a_cell_keeps_the_two_colors_that_lose_least() {
    let (red, blue, white) = ([200, 0, 0], [0, 0, 200], [255; 3]);
    let (fg, bg, bits) = split(&[red, blue, blue, white, blue, red]);
    // Dropping the one white pixel costs less than dropping either red.
    assert_eq!((fg, bg), (red, blue));
    assert_eq!(bits & 0b100001, 0b100001);
    assert_eq!(split(&[blue; 6]), (blue, blue, 0));
    assert_eq!(split(&[]), ([0; 3], [0; 3], 0));
}

#[test]
fn the_image_measures_its_cells_and_resizes_in_place() {
    let mut pixels = Pixels::new(5, 5);
    assert_eq!(pixels.cells(), (5, 3));
    pixels.blocks = Blocks::Sextant;
    assert_eq!(pixels.cells(), (3, 2));
    pixels.set(4, 4, [1, 2, 3]);
    pixels.resize(6, 6);
    assert_eq!(pixels.get(4, 4), Some([1, 2, 3]));
    assert_eq!(pixels.get(5, 5), Some([0, 0, 0]));
    pixels.write(&[9, 8, 7, 255, 6, 5, 4, 0]);
    assert_eq!(
        (pixels.get(0, 0), pixels.get(1, 0)),
        (Some([9, 8, 7]), Some([6, 5, 4]))
    );
    let mut screen = Screen::new(2, 1);
    show(&mut screen, pixels);
    // Given fewer cells than it needs, the image is clipped, not scaled: the
    // first cell holds the two written pixels, the second only black ones.
    let frame = screen.frame().unwrap();
    assert_ne!(frame.cell(0, 0).unwrap().symbol(), " ");
    assert_eq!(frame.cell(1, 0).unwrap().symbol(), " ");
}
