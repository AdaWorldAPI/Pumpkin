use criterion::{Criterion, black_box, criterion_group, criterion_main};
use pumpkin::entity::Entity;
use pumpkin::entity::compute_collision_math;
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;

fn criterion_benchmark(c: &mut Criterion) {
    let bbox = BoundingBox::new_from_pos(
        0.0,
        0.0,
        0.0,
        &pumpkin::entity::Entity::get_entity_dimensions(pumpkin::entity::EntityPose::Standing),
    );
    let movement = Vector3::new(1.0, 0.0, 1.0);

    let collisions = vec![
        BoundingBox::new_from_pos(
            0.5,
            0.0,
            0.5,
            &pumpkin::entity::Entity::get_entity_dimensions(pumpkin::entity::EntityPose::Standing),
        ),
        BoundingBox::new_from_pos(
            0.5,
            1.0,
            0.5,
            &pumpkin::entity::Entity::get_entity_dimensions(pumpkin::entity::EntityPose::Standing),
        ),
    ];

    let block_positions = vec![
        (0usize, BlockPos::new(0, 0, 0)),
        (1usize, BlockPos::new(0, 1, 0)),
    ];

    c.bench_function("collision_math_hotpath", |b| {
        b.iter(|| {
            compute_collision_math(
                black_box(movement),
                black_box(bbox.clone()),
                black_box(collisions.clone()),
                black_box(block_positions.clone()),
            )
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
