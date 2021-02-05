use itertools::Itertools;
#[allow(unused_imports)]
use log::{debug, error, info, log_enabled};
use rand::distributions::{Distribution, Uniform};

pub(crate) struct Maze {
    pub width: usize,
    pub height: usize,
    temp_maze: Vec<Vec<WallStatus>>,
}

pub trait TriangleIndexList<T> {
    fn new() -> Self;
    fn push(&mut self, p0: T, p1: T, p2: T);
}

pub trait VertexList<T>: Sized {
    fn new() -> Self;
    fn with_capacity(_capacity: usize) -> Self {
        Self::new()
    }
    fn push(&mut self, p0: T, p1: T);
}

impl Maze {
    /// Create a new std maze with specified Rng
    pub fn new<R: rand::Rng>(mut rng: &mut R) -> Maze {
        let width = rng.gen_range(4..13);
        let height = rng.gen_range(4..11);

        let between = Uniform::from(0..4);
        let temp_maze = (0..)
            .map(|_| {
                between
                    .sample_iter(&mut rng)
                    .map(|num| match num {
                        0 => WallStatus::Top,
                        1 => WallStatus::Right,
                        2 => WallStatus::Bottom,
                        3 => WallStatus::Left,
                        _ => unreachable!(),
                    })
                    .take(width + 1)
                    .collect()
            })
            .take(height + 1)
            .collect();
        debug!("Created maze: [{}, {}]", width, height);
        Maze {
            width,
            height,
            temp_maze,
        }
    }

    pub fn triangle_mesh<V, I>(&self) -> (V, I)
        where
            V: VertexList<f32>,
            I: TriangleIndexList<u32>,
    {
        const FRAC_1_16: f32 = 1.0 / 16.0;
        // Generate vertices, 4 vertices for each point.
        let mut vertices = V::with_capacity(self.width * self.height * 4);
        for y in 0..=self.height {
            for x in 0..=self.width {
                let x = x as f32 + 0.5 - self.width as f32 / 2.0;
                let y = y as f32 + 0.5 - self.height as f32 / 2.0;
                vertices.push(x - FRAC_1_16, y - FRAC_1_16);
                vertices.push(x + FRAC_1_16, y - FRAC_1_16);
                vertices.push(x - FRAC_1_16, y + FRAC_1_16);
                vertices.push(x + FRAC_1_16, y + FRAC_1_16);
            }
        }

        // Generate indices
        let get_offset = |x, y| {
            (4 * (x + y * (self.width + 1))..)
                .map(|v| v as u32)
                .take(4)
                .collect_tuple()
                .unwrap()
        };

        let mut indexes = I::new();
        for y in 0..=self.height {
            for x in 0..=self.width {
                let (p0, p1, p2, _) = get_offset(x, y);
                if x < self.width
                    && (y == 0
                    || y == self.height
                    || self.temp_maze[y][x + 1] == WallStatus::Bottom
                    || self.temp_maze[y][x] == WallStatus::Top)
                {
                    let (_, n1, _, n3) = get_offset(x + 1, y);
                    indexes.push(p0, n1, n3);
                    indexes.push(p0, n3, p2);
                }
                if y < self.height
                    && (x == 0
                    || x == self.width
                    || self.temp_maze[y + 1][x] == WallStatus::Right
                    || self.temp_maze[y][x] == WallStatus::Left)
                {
                    let (_, _, n2, n3) = get_offset(x, y + 1);
                    indexes.push(p0, p1, n3);
                    indexes.push(p0, n3, n2);
                }
            }
        }

        (vertices, indexes)
    }
}

#[derive(Ord, PartialOrd, Eq, PartialEq)]
enum WallStatus {
    Top,
    Right,
    Bottom,
    Left,
}
