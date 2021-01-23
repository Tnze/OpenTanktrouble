use rand::Rng;

fn new_maze() {
    let size_x = rand::thread_rng().gen_range(7..13);
    let size_y = rand::thread_rng().gen_range(3..10);
}