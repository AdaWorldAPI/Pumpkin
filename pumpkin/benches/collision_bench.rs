use criterion::{Criterion, criterion_group, criterion_main};
use pumpkin::entity::{Entity, compute_collision_math};
use pumpkin_data::entity::EntityPose;
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use std::hint::black_box;

fn criterion_benchmark(c: &mut Criterion) {
    let bbox = BoundingBox::new_from_pos(
        0.0,
        0.0,
        0.0,
        &Entity::get_entity_dimensions(EntityPose::Standing),
    );
    let movement = Vector3::new(1.0, 0.0, 1.0);

    let collisions = vec![
        BoundingBox::new_from_pos(
            0.5,
            0.0,
            0.5,
            &Entity::get_entity_dimensions(EntityPose::Standing),
        ),
        BoundingBox::new_from_pos(
            0.5,
            1.0,
            0.5,
            &Entity::get_entity_dimensions(EntityPose::Standing),
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
                black_box(bbox),
                black_box(collisions.clone()),
                black_box(block_positions.clone()),
            )
        });
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
