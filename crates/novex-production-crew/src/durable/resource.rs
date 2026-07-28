use super::{domain_error, plan::ResourceLimits};
use crate::{ProductionError, ProductionResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequest {
    pub values: BTreeMap<String, u64>,
}

impl ResourceRequest {
    pub fn role_call(input_tokens: u64, output_tokens: u64) -> Self {
        Self {
            values: BTreeMap::from([
                ("role_calls".into(), 1),
                ("input_tokens".into(), input_tokens),
                ("output_tokens".into(), output_tokens),
            ]),
        }
    }

    pub fn video_generation(tasks: u64, duration_sec: u64) -> Self {
        Self {
            values: BTreeMap::from([
                ("video_tasks".into(), tasks),
                ("video_duration_sec".into(), duration_sec),
            ]),
        }
    }

    pub fn role_retry() -> Self {
        Self {
            values: BTreeMap::from([("role_retries".into(), 1)]),
        }
    }

    pub fn quality_rework() -> Self {
        Self {
            values: BTreeMap::from([("quality_reworks".into(), 1)]),
        }
    }

    pub fn work_generation(
        video_tasks: u64,
        video_duration_sec: u64,
        tts_characters: u64,
        asr_tasks: u64,
        concurrency: u64,
    ) -> Self {
        Self {
            values: BTreeMap::from([
                ("video_tasks".into(), video_tasks),
                ("video_duration_sec".into(), video_duration_sec),
                ("tts_characters".into(), tts_characters),
                ("asr_tasks".into(), asr_tasks),
                ("concurrency".into(), concurrency),
            ]),
        }
    }

    pub fn provider_retry(values: BTreeMap<String, u64>) -> Self {
        let mut request = values;
        request.insert("provider_retries".into(), 1);
        Self { values: request }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReservationStatus {
    Reserved,
    Settled,
    Released,
    HeldUncertain,
}

#[derive(Debug, Clone)]
pub struct ResourceReservation {
    pub id: Uuid,
    pub values: BTreeMap<String, u64>,
    pub status: ReservationStatus,
}

pub struct ResourceUsageLedger {
    limits: ResourceLimits,
    reservations: BTreeMap<Uuid, ResourceReservation>,
    actual: BTreeMap<String, u64>,
}

impl ResourceUsageLedger {
    pub fn new(limits: ResourceLimits) -> Self {
        Self {
            limits,
            reservations: BTreeMap::new(),
            actual: BTreeMap::new(),
        }
    }

    fn active_value(&self, key: &str, include_uncertain: bool) -> u64 {
        self.reservations
            .values()
            .filter(|reservation| {
                reservation.status == ReservationStatus::Reserved
                    || (include_uncertain && reservation.status == ReservationStatus::HeldUncertain)
            })
            .map(|reservation| reservation.values.get(key).copied().unwrap_or(0))
            .sum()
    }

    pub fn reserved(&self, key: &str) -> u64 {
        self.active_value(key, false)
    }

    pub fn held_uncertain(&self, key: &str) -> u64 {
        self.reservations
            .values()
            .filter(|reservation| reservation.status == ReservationStatus::HeldUncertain)
            .map(|reservation| reservation.values.get(key).copied().unwrap_or(0))
            .sum()
    }

    pub fn actual(&self, key: &str) -> u64 {
        self.actual.get(key).copied().unwrap_or(0)
    }
}

pub struct ResourceSafetyGate;

impl ResourceSafetyGate {
    pub fn reserve(
        ledger: &mut ResourceUsageLedger,
        request: ResourceRequest,
    ) -> ProductionResult<ResourceReservation> {
        for (key, requested) in &request.values {
            let Some(limit) = ledger.limits.value(key) else {
                return Err(domain_error(format!("unknown resource key: {key}")));
            };
            let actual = if key == "concurrency" {
                0
            } else {
                ledger.actual(key)
            };
            let current = actual + ledger.active_value(key, true);
            if current.saturating_add(*requested) > limit {
                return Err(ProductionError::ResourceLimit {
                    resource: key.clone(),
                    current,
                    requested: *requested,
                    limit,
                });
            }
        }
        let reservation = ResourceReservation {
            id: Uuid::new_v4(),
            values: request.values,
            status: ReservationStatus::Reserved,
        };
        ledger
            .reservations
            .insert(reservation.id, reservation.clone());
        Ok(reservation)
    }

    pub fn settle(
        ledger: &mut ResourceUsageLedger,
        reservation_id: Uuid,
        actual_primary_value: Option<u64>,
        result_uncertain: bool,
    ) -> ProductionResult<()> {
        let Some(reservation) = ledger.reservations.get_mut(&reservation_id) else {
            return Err(domain_error("resource reservation not found"));
        };
        if reservation.status != ReservationStatus::Reserved {
            return Err(domain_error("resource reservation already terminal"));
        }
        if result_uncertain {
            reservation.status = ReservationStatus::HeldUncertain;
            return Ok(());
        }

        let primary_key = if reservation.values.contains_key("input_tokens") {
            Some("input_tokens")
        } else if reservation.values.contains_key("video_duration_sec") {
            Some("video_duration_sec")
        } else {
            reservation.values.keys().next().map(String::as_str)
        };
        for (key, reserved) in &reservation.values {
            let actual = if Some(key.as_str()) == primary_key {
                actual_primary_value.unwrap_or(*reserved)
            } else {
                *reserved
            };
            *ledger.actual.entry(key.clone()).or_default() += actual;
        }
        reservation.status = ReservationStatus::Settled;
        Ok(())
    }

    pub fn release(ledger: &mut ResourceUsageLedger, reservation_id: Uuid) -> ProductionResult<()> {
        let Some(reservation) = ledger.reservations.get_mut(&reservation_id) else {
            return Err(domain_error("resource reservation not found"));
        };
        if reservation.status != ReservationStatus::Reserved {
            return Err(domain_error("only unused reservations may be released"));
        }
        reservation.status = ReservationStatus::Released;
        Ok(())
    }
}
