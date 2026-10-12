use tg_policy::ConfidentialityLevel;

/// Ziel-Adapter für einen Gradienten-Schritt.
///
/// `SharedLogic` entspricht W_logik (geteilter Adapter, wird gefaltet zum Hive),
/// `PrivateLocal` entspricht W_PI (privater Adapter, verlässt das Gerät nie).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterTarget {
    SharedLogic,
    PrivateLocal,
}

/// Maskiert Gradienten nach Vertraulichkeitsstufe (REQ-SYS-003):
/// Streng geheim und Nur lokal aktualisieren ausschließlich den privaten Adapter.
pub fn mask_gradients(example_level: ConfidentialityLevel) -> AdapterTarget {
    match example_level {
        ConfidentialityLevel::StrengGeheim | ConfidentialityLevel::NurLokal => {
            AdapterTarget::PrivateLocal
        }
        _ => AdapterTarget::SharedLogic,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AdapterWeights {
    pub values: Vec<f32>,
}

impl AdapterWeights {
    pub fn new(size: usize) -> Self {
        Self {
            values: vec![0.0; size],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrainSummary {
    pub shared_updates: usize,
    pub private_updates: usize,
}

/// Lokaler Trainingsschritt über LoRA-Adapter.
///
/// Die Basisgewichte werden nie verändert – sie sind in diesem Skeleton bewusst
/// nicht Teil der Signatur. Je Beispiel wird der Gradient per `mask_gradients`
/// in den passenden Adapter geroutet.
pub fn train_local(
    shared: &mut AdapterWeights,
    private: &mut AdapterWeights,
    examples: &[(ConfidentialityLevel, f32)],
    learning_rate: f32,
) -> TrainSummary {
    let mut summary = TrainSummary {
        shared_updates: 0,
        private_updates: 0,
    };

    for (level, gradient) in examples {
        match mask_gradients(*level) {
            AdapterTarget::SharedLogic => {
                for value in shared.values.iter_mut() {
                    *value -= learning_rate * gradient;
                }
                summary.shared_updates += 1;
            }
            AdapterTarget::PrivateLocal => {
                for value in private.values.iter_mut() {
                    *value -= learning_rate * gradient;
                }
                summary.private_updates += 1;
            }
        }
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::{mask_gradients, train_local, AdapterTarget, AdapterWeights};
    use tg_policy::ConfidentialityLevel;

    #[test]
    fn strict_and_local_examples_route_to_private_adapter() {
        assert_eq!(
            mask_gradients(ConfidentialityLevel::StrengGeheim),
            AdapterTarget::PrivateLocal
        );
        assert_eq!(
            mask_gradients(ConfidentialityLevel::NurLokal),
            AdapterTarget::PrivateLocal
        );
        assert_eq!(
            mask_gradients(ConfidentialityLevel::Offen),
            AdapterTarget::SharedLogic
        );
        assert_eq!(
            mask_gradients(ConfidentialityLevel::Vertraulich),
            AdapterTarget::SharedLogic
        );
    }

    #[test]
    fn train_local_updates_only_the_target_adapter() {
        let mut shared = AdapterWeights::new(4);
        let mut private = AdapterWeights::new(4);
        let examples = [
            (ConfidentialityLevel::Offen, 1.0_f32),
            (ConfidentialityLevel::NurLokal, 2.0_f32),
        ];

        let summary = train_local(&mut shared, &mut private, &examples, 0.1);

        assert_eq!(summary.shared_updates, 1);
        assert_eq!(summary.private_updates, 1);
        assert!(shared.values.iter().all(|value| (*value - -0.1).abs() < 1e-6));
        assert!(private.values.iter().all(|value| (*value - -0.2).abs() < 1e-6));
    }

    #[test]
    fn train_local_with_empty_examples_changes_nothing() {
        let mut shared = AdapterWeights::new(2);
        let mut private = AdapterWeights::new(2);

        let summary = train_local(&mut shared, &mut private, &[], 0.1);

        assert_eq!(summary.shared_updates, 0);
        assert_eq!(summary.private_updates, 0);
        assert_eq!(shared.values, vec![0.0, 0.0]);
        assert_eq!(private.values, vec![0.0, 0.0]);
    }
}
