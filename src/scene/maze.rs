use futures::TryFutureExt;
use itertools::Itertools;
use rand::distributions::{Distribution, Uniform};
use rapier2d::{math::Point, na::Point3};

use super::playground::Vertex;

pub(crate) struct Maze {
    pub width: usize,
    pub height: usize,
    temp_maze: Vec<Vec<WallStatus>>,
}

pub trait TripletPointList<T> {
    fn push(&mut self, p0: T, p1: T, p2: T);
}

impl TripletPointList<u32> for Vec<Point3<u32>> {
    fn push(&mut self, p0: u32, p1: u32, p2: u32) {
        self.push(Point3::new(p0, p1, p2));
    }
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
                    .take(width)
                    .collect()
            })
            .take(height)
            .collect();

        Maze {
            width,
            height,
            temp_maze,
        }
    }

    pub fn triangle_mesh(&self, indices: &mut dyn TripletPointList<u32>) -> Vec<Vertex> {
        const FRAC_1_16: f32 = 1.0 / 16.0;
        // Generate vertices, 4 vertices for each point.
        let mut vertices = Vec::with_capacity(4 * self.width * self.height);
        for y in 0..self.height {
            for x in 0..self.width {
                vertices.push(Vertex::new(x as f32 - FRAC_1_16, y as f32 - FRAC_1_16));
                vertices.push(Vertex::new(x as f32 + FRAC_1_16, y as f32 - FRAC_1_16));
                vertices.push(Vertex::new(x as f32 - FRAC_1_16, y as f32 + FRAC_1_16));
                vertices.push(Vertex::new(x as f32 + FRAC_1_16, y as f32 + FRAC_1_16));
            }
        }
        // Generate indices
        let get_offset = |x, y| {
            (4 * (x + y * self.width)..)
                .map(|v| v as u32)
                .take(4)
                .collect_tuple()
                .unwrap()
        };

        for y in 0..(self.height - 1) {
            for x in 0..(self.width - 1) {
                let (p0, p1, p2, _) = get_offset(x, y);
                if y == 0
                    || self.temp_maze[y - 1][x] == WallStatus::Bottom
                    || self.temp_maze[y][x] == WallStatus::Top
                {
                    let (_, n1, _, n3) = get_offset(x + 1, y);
                    indices.push(p0, n3, n1);
                    indices.push(p0, p2, n3);
                }
                if x == 0
                    || self.temp_maze[y][x - 1] == WallStatus::Right
                    || self.temp_maze[y][x] == WallStatus::Left
                {
                    let (_, _, n2, n3) = get_offset(x, y + 1);
                    indices.push(p0, n3, p1);
                    indices.push(p0, n2, n3);
                }
            }
        }
        //TODO: Add the bottom and right wall
        vertices
    }
}

#[derive(Ord, PartialOrd, Eq, PartialEq)]
enum WallStatus {
    Top,
    Right,
    Bottom,
    Left,
}

#[cfg(test)]
mod tests {
    use super::Maze;

    #[test]
    fn generate() {
        // testing if it panic;
        let mut rng = rand::thread_rng();
        for _ in (0..).map(|_| Maze::new(&mut rng)).take(10000) {}
    }

    #[test]
    fn gen_mesh() {
        // testing if it panic;
        let mut rng = rand::thread_rng();
        let maze = Maze::new(&mut rng);
        let mut list = Vec::new();
        maze.triangle_mesh(&mut list);
    }
}
