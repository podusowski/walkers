//! Builds Walkers' own icons, for `lookup_icon_outline`, from the SVG files in `icons/`.

fn main() {
    // Without it, Cargo runs this again whenever any file of the package changes.
    println!("cargo::rerun-if-changed=icons");

    #[cfg(feature = "vector")]
    icons::generate();
}

#[cfg(feature = "vector")]
mod icons {
    use std::fmt::Write as _;
    use std::path::Path;

    use lyon_tessellation::{
        BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, VertexBuffers,
        math::{Point, point},
        path::{PathEvent, iterator::PathIterator},
    };
    use usvg::tiny_skia_path::PathSegment;

    /// How far the lines a curve is drawn with may stray from it, as a part of the icon's width.
    /// At 64 pixels across, a tenth of a pixel.
    const TOLERANCE: f32 = 0.0015;

    type Error = Box<dyn std::error::Error>;

    /// An icon, read from its file, centred on the origin and one across.
    struct Icon {
        /// As a style names it, which is the name of its file, such as `bus_stop`.
        name: String,
        /// For its edges.
        rings: Vec<Vec<Point>>,
        /// For its inside.
        vertices: Vec<Point>,
        indices: Vec<u32>,
    }

    pub fn generate() {
        let mut icons: Vec<Icon> = std::fs::read_dir("icons")
            .expect("icons/ to be readable")
            .map(|entry| entry.expect("icons/ to be readable").path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "svg"))
            // Panicking is how a build script fails.
            .map(|path| read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display())))
            .collect();
        icons.sort_by(|a, b| a.name.cmp(&b.name));

        let out_dir = std::env::var("OUT_DIR").expect("Cargo to set OUT_DIR");
        std::fs::write(Path::new(&out_dir).join("icons.rs"), code(&icons))
            .expect("OUT_DIR to be writable");
    }

    /// Fails for an SVG which Walkers could not draw as an editor shows it.
    fn read(path: &Path) -> Result<Icon, Error> {
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or("its name is not UTF-8")?;

        let tree =
            usvg::Tree::from_str(&std::fs::read_to_string(path)?, &usvg::Options::default())?;

        let size = tree.size();
        if size.width() != size.height() {
            return Err("an icon is square, so its viewBox is too".into());
        }

        let mut paths = Vec::new();
        collect_paths(tree.root(), &mut paths)?;
        if paths.is_empty() {
            return Err("draws nothing".into());
        }

        // Each path is filled on its own, as an SVG is, so that two overlapping do not cut a
        // hole in each other.
        let mut rings = Vec::new();
        let mut buffers: VertexBuffers<Point, u32> = VertexBuffers::new();
        for (svg_path, fill_rule) in paths {
            // In the viewBox's units, transforms applied.
            let to_unit = |mut p: usvg::tiny_skia_path::Point| {
                svg_path.abs_transform().map_point(&mut p);
                point(p.x / size.width() - 0.5, p.y / size.height() - 0.5)
            };
            let mut builder = lyon_tessellation::path::Path::svg_builder();
            for segment in svg_path.data().segments() {
                match segment {
                    PathSegment::MoveTo(p) => _ = builder.move_to(to_unit(p)),
                    PathSegment::LineTo(p) => _ = builder.line_to(to_unit(p)),
                    PathSegment::QuadTo(c, p) => {
                        _ = builder.quadratic_bezier_to(to_unit(c), to_unit(p))
                    }
                    PathSegment::CubicTo(c1, c2, p) => {
                        _ = builder.cubic_bezier_to(to_unit(c1), to_unit(c2), to_unit(p))
                    }
                    PathSegment::Close => builder.close(),
                }
            }
            let lyon_path = builder.build();

            for event in lyon_path.iter().flattened(TOLERANCE) {
                match event {
                    PathEvent::Begin { at } => rings.push(vec![at]),
                    PathEvent::Line { to, .. } => rings
                        .last_mut()
                        .expect("a line to begin somewhere")
                        .push(to),
                    _ => {}
                }
            }

            FillTessellator::new().tessellate_path(
                &lyon_path,
                &FillOptions::tolerance(TOLERANCE).with_fill_rule(fill_rule),
                &mut BuffersBuilder::new(&mut buffers, |vertex: FillVertex| vertex.position()),
            )?;
        }

        // A label makes room for its icon by the icon's size.
        if rings
            .iter()
            .flatten()
            .any(|p| p.x.abs() > 0.5 || p.y.abs() > 0.5)
        {
            return Err("reaches out of its viewBox".into());
        }

        Ok(Icon {
            name: name.to_owned(),
            rings,
            vertices: buffers.vertices,
            indices: buffers.indices,
        })
    }

    /// The paths of `group` and of the groups in it, each with how it is filled. A stroke or a
    /// gradient would make the icon look other than it does in an editor, so it is refused
    /// rather than ignored. The colour is the style's.
    fn collect_paths<'a>(
        group: &'a usvg::Group,
        paths: &mut Vec<(&'a usvg::Path, FillRule)>,
    ) -> Result<(), Error> {
        for node in group.children() {
            match node {
                usvg::Node::Group(group) => collect_paths(group, paths)?,
                usvg::Node::Path(path) if !path.is_visible() => {}
                usvg::Node::Path(path) => {
                    if path.stroke().is_some() {
                        return Err("a stroke is not drawn; turn it into a path".into());
                    }
                    // Without one, such as a guide, it draws nothing.
                    let Some(fill) = path.fill() else {
                        continue;
                    };
                    if !matches!(fill.paint(), usvg::Paint::Color(_)) {
                        return Err("an icon is filled in the one colour the style gives it, \
                                    not with a gradient or pattern"
                            .into());
                    }
                    paths.push((
                        path,
                        match fill.rule() {
                            usvg::FillRule::NonZero => FillRule::NonZero,
                            usvg::FillRule::EvenOdd => FillRule::EvenOdd,
                        },
                    ));
                }
                usvg::Node::Image(_) | usvg::Node::Text(_) => {
                    return Err("an icon is made of shapes, not of images or text".into());
                }
            }
        }
        Ok(())
    }

    /// An `icons!` of `icon.rs`, listing each icon.
    fn code(icons: &[Icon]) -> String {
        let points = |points: &[Point]| {
            points
                .iter()
                .map(|p| format!("pos2({:?}, {:?})", p.x, p.y))
                .collect::<Vec<_>>()
                .join(", ")
        };

        let mut code = String::from("// Generated by build.rs from icons/*.svg.\n\nicons! {\n");
        for icon in icons {
            let rings: Vec<_> = icon
                .rings
                .iter()
                .map(|ring| format!("&[{}]", points(ring)))
                .collect();
            writeln!(
                code,
                "    {name:?} => {{\n        rings: &[{rings}],\n        \
                 vertices: &[{vertices}],\n        indices: &{indices:?},\n    }}",
                name = icon.name,
                rings = rings.join(", "),
                vertices = points(&icon.vertices),
                indices = icon.indices,
            )
            .expect("a String to be writable");
        }
        code + "}\n"
    }
}
