use itertools::Itertools;
use rand::distributions::{Distribution, Uniform};
use rapier2d::{math::Point, na::Point3};

pub(crate) struct Maze {
    width: usize,
    height: usize,
    temp_maze: Vec<Vec<WallStatus>>,
}

impl Maze {
    /// Create a new std maze with specified Rng
    fn new<R: rand::Rng>(mut rng: &mut R) -> Maze {
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

    fn triangle_mesh(&self) {
        const FRAC_1_16: f32 = 1.0 / 16.0;
        // Generate vertices, 4 vertices for each point.
        let mut vertices = Vec::with_capacity((4 * self.width * self.height) as usize);
        for (x, y) in (0..self.width).zip(0..self.height) {
            vertices.push(Point::new(x as f32 - FRAC_1_16, y as f32 - FRAC_1_16));
            vertices.push(Point::new(x as f32 + FRAC_1_16, y as f32 - FRAC_1_16));
            vertices.push(Point::new(x as f32 - FRAC_1_16, y as f32 + FRAC_1_16));
            vertices.push(Point::new(x as f32 + FRAC_1_16, y as f32 + FRAC_1_16));
        }
        // Generate indices
        let get_offset = |x, y| (4 * (x + y * self.width)..).take(4).collect_tuple().unwrap();
        let mut indices = Vec::new();
        for (x, y) in (0..self.width).zip(0..self.height) {
            let (p0, p1, p2, _) = get_offset(x, y);
            if y == 0
                || self.temp_maze[x][y - 1] == WallStatus::Bottom
                || self.temp_maze[x][y] == WallStatus::Top
            {
                let (_, n1, _, n3) = get_offset(x + 1, y);
                indices.push(Point3::new(p0, n3, n1));
                indices.push(Point3::new(p0, p2, n3));
            }
            if x == 0
                || self.temp_maze[x - 1][y] == WallStatus::Right
                || self.temp_maze[x][y] == WallStatus::Left
            {
                let (_, _, n2, n3) = get_offset(x, y + 1);
                indices.push(Point3::new(p0, n3, p1));
                indices.push(Point3::new(p0, n2, n3));
            }
        }
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
        maze.triangle_mesh();
    }
}
