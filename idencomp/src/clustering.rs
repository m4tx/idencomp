use itertools::Itertools;
use log::trace;
use rand::prelude::SliceRandom;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;

#[derive(Debug)]
pub(crate) struct Clustering {
    rand: Xoshiro256PlusPlus,
}

impl Clustering {
    #[must_use]
    pub fn new() -> Self {
        Self {
            rand: Xoshiro256PlusPlus::seed_from_u64(404),
        }
    }

    #[must_use]
    pub fn make_clusters<'a, Value, Centroid, Calculator>(
        &mut self,
        mut cost_calculator: Calculator,
        centroids: &'a [Centroid],
        values: &'a [Value],
        num_clusters: usize,
    ) -> Vec<Cluster>
    where
        Calculator: ClusterCostCalculator<Value, Centroid>,
    {
        if num_clusters == 0 {
            return Vec::new();
        }
        let num_clusters = num_clusters.min(centroids.len());

        let mut best_centroids = Vec::with_capacity(num_clusters);
        // This is to disallow more than one cluster with the same centroid
        let mut centroids_available = vec![true; centroids.len()];
        let mut value_clusters = vec![0; values.len()];

        for value in values.choose_multiple(&mut self.rand, num_clusters) {
            let best_centroid = Self::best_centroid_for(
                &mut cost_calculator,
                centroids,
                &centroids_available,
                [value],
            );

            best_centroids.push(best_centroid);
            centroids_available[best_centroid] = false;
        }
        trace!("Initial centroids: {:?}", best_centroids);

        loop {
            let mut cluster_changes = 0;
            let mut centroid_changes = 0;

            for (value_index, value) in values.iter().enumerate() {
                let new_cluster_index = best_centroids
                    .iter()
                    .enumerate()
                    .map(|(cluster_index, &centroid_index)| {
                        let cost = cost_calculator.cost_for(value, &centroids[centroid_index]);
                        (cluster_index, cost)
                    })
                    .sorted_by_key(|(_cluster_index, cost)| *cost)
                    .next()
                    .unwrap()
                    .0;

                if value_clusters[value_index] != new_cluster_index {
                    value_clusters[value_index] = new_cluster_index;
                    cluster_changes += 1;
                }
            }

            for flag in centroids_available.iter_mut() {
                *flag = true;
            }
            for (cluster_index, centroid_index) in best_centroids.iter_mut().enumerate() {
                let cluster_values = Self::cluster_values(&value_clusters, cluster_index)
                    .map(|index| &values[index]);
                let best_centroid = Self::best_centroid_for(
                    &mut cost_calculator,
                    centroids,
                    &centroids_available,
                    cluster_values,
                );

                if *centroid_index != best_centroid {
                    *centroid_index = best_centroid;
                    centroid_changes += 1;
                }
                centroids_available[best_centroid] = false;
            }

            trace!("Current centroids: {:?}", best_centroids);
            trace!(
                "Cluster changes: {}; Centroid changed: {}",
                cluster_changes,
                centroid_changes
            );
            if cluster_changes == 0 && centroid_changes == 0 {
                trace!("Converged");
                break;
            }
        }

        best_centroids
            .into_iter()
            .enumerate()
            .map(|(cluster_index, best_centroid)| {
                let cluster_values: Vec<usize> =
                    Self::cluster_values(&value_clusters, cluster_index).collect();
                Cluster::new(best_centroid, cluster_values)
            })
            .collect()
    }

    fn cluster_values(
        value_clusters: &[usize],
        cluster_index: usize,
    ) -> impl Iterator<Item = usize> + '_ {
        value_clusters
            .iter()
            .enumerate()
            .filter(move |(_value_index, &value_cluster)| value_cluster == cluster_index)
            .map(|(value_index, &_value_cluster)| value_index)
    }

    #[must_use]
    fn best_centroid_for<'a, Value: 'a, Centroid, Calculator, I>(
        cost_calculator: &mut Calculator,
        centroids: &'a [Centroid],
        centroids_available: &'a [bool],
        values: I,
    ) -> usize
    where
        Calculator: ClusterCostCalculator<Value, Centroid>,
        I: IntoIterator<Item = &'a Value>,
    {
        let mut centroid_costs = vec![0; centroids.len()];
        for value in values {
            for (index, centroid) in centroids.iter().enumerate() {
                centroid_costs[index] += cost_calculator.cost_for(value, centroid);
            }
        }

        centroid_costs
            .into_iter()
            .enumerate()
            .filter(|(index, _cost)| centroids_available[*index])
            .sorted_by_key(|(_index, cost)| *cost)
            .next()
            .unwrap()
            .0
    }
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) struct Cluster {
    pub centroid: usize,
    pub values: Vec<usize>,
}

impl Cluster {
    #[must_use]
    pub fn new<T: Into<Vec<usize>>>(centroid: usize, values: T) -> Self {
        Self {
            centroid,
            values: values.into(),
        }
    }
}

pub(crate) trait ClusterCostCalculator<Value, Centroid> {
    fn cost_for(&mut self, value: &Value, centroid: &Centroid) -> u32;
}

#[cfg(test)]
mod tests {
    use crate::clustering::{Cluster, ClusterCostCalculator, Clustering};

    #[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
    struct Point {
        x: i32,
        y: i32,
    }

    impl Point {
        fn new(x: i32, y: i32) -> Self {
            Self { x, y }
        }
    }

    struct PointCostCalculator {}

    impl ClusterCostCalculator<Point, Point> for PointCostCalculator {
        fn cost_for(&mut self, value: &Point, centroid: &Point) -> u32 {
            let x_cost = value.x.abs_diff(centroid.x).pow(2);
            let y_cost = value.y.abs_diff(centroid.y).pow(2);
            x_cost + y_cost
        }
    }

    #[test_log::test]
    fn test_cluster_trivial() {
        let point = Point::new(0, 0);
        let points = vec![point];
        let centroids = vec![
            Point::new(2, 1),
            Point::new(-2, 2),
            point,
            Point::new(3, -3),
        ];

        let calculator = PointCostCalculator {};
        let mut clustering = Clustering::new();
        let clusters = clustering.make_clusters(calculator, &centroids, &points, 1);

        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].centroid, 2);
        assert_eq!(clusters[0].values.len(), 1);
        assert_eq!(clusters[0].values[0], 0);
    }

    #[test_log::test]
    fn test_cluster_points() {
        let cluster_1 = vec![Point::new(2, 2), Point::new(2, 3), Point::new(4, 1)];
        let cluster_2 = vec![Point::new(-1, 1), Point::new(-2, 1), Point::new(-3, 2)];
        let cluster_3 = vec![Point::new(-2, -2)];
        let cluster_4 = vec![Point::new(2, -2), Point::new(2, -3)];

        let cluster_1_centroid = Point::new(2, 1);
        let cluster_2_centroid = Point::new(-2, 2);
        let cluster_3_centroid = Point::new(-1, -1);
        let cluster_4_centroid = Point::new(3, -3);
        let additional_centroid_1 = Point::new(-6, -7);
        let additional_centroid_2 = Point::new(0, 0);

        let points: Vec<Point> = cluster_1
            .iter()
            .chain(cluster_2.iter())
            .chain(cluster_3.iter())
            .chain(cluster_4.iter())
            .cloned()
            .collect();
        let centroids = vec![
            additional_centroid_1,
            additional_centroid_2,
            cluster_1_centroid,
            cluster_2_centroid,
            cluster_3_centroid,
            cluster_4_centroid,
        ];

        let calculator = PointCostCalculator {};
        let mut clustering = Clustering::new();
        let mut clusters = clustering.make_clusters(calculator, &centroids, &points, 4);
        clusters.sort();

        assert_eq!(
            clusters,
            [
                Cluster::new(2, [0, 1, 2]),
                Cluster::new(3, [3, 4, 5]),
                Cluster::new(4, [6]),
                Cluster::new(5, [7, 8]),
            ]
        );
    }
}
