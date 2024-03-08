use crate::config::{config, Config};
use crate::jobs::constants::JOB_PROCESS_ATTEMPT_METADATA_KEY;
use crate::jobs::types::{JobItem, JobStatus, JobType, JobVerificationStatus};
use crate::queue::job_queue::{add_job_to_process_queue, add_job_to_verification_queue};
use async_trait::async_trait;
use color_eyre::eyre::eyre;
use color_eyre::Result;
use std::collections::HashMap;
use std::time::Duration;
use tracing::log;
use uuid::Uuid;

mod constants;
pub mod da_job;
pub mod types;

#[async_trait]
pub trait Job: Send + Sync {
    async fn create_job(&self, config: &Config, internal_id: String) -> Result<JobItem>;
    async fn process_job(&self, config: &Config, job: &JobItem) -> Result<String>;
    async fn verify_job(&self, config: &Config, job: &JobItem) -> Result<JobVerificationStatus>;
    fn max_process_attempts(&self) -> u64;
    fn max_verification_attempts(&self) -> u64;
    fn verification_polling_delay_seconds(&self) -> u64;
}

pub async fn create_job(job_type: JobType, internal_id: String) -> Result<()> {
    let config = config().await;
    let existing_job = config.database().get_job_by_internal_id_and_type(&internal_id, &job_type).await?;
    if existing_job.is_some() {
        log::debug!("Job already exists for internal_id {:?} and job_type {:?}. Skipping.", internal_id, job_type);
        return Err(eyre!(
            "Job already exists for internal_id {:?} and job_type {:?}. Skipping.",
            internal_id,
            job_type
        ));
    }

    let job_handler = get_job_handler(&job_type);
    let job_item = job_handler.create_job(config, internal_id).await?;
    config.database().create_job(job_item.clone()).await?;

    add_job_to_process_queue(job_item.id).await?;
    Ok(())
}

pub async fn process_job(id: Uuid) -> Result<()> {
    let config = config().await;
    let job = get_job(id).await?;

    match job.status {
        // we only want to process jobs that are in the created or verification failed state.
        // verification failed state means that the previous processing failed and we want to retry
        JobStatus::Created | JobStatus::VerificationFailed => {
            log::info!("Processing job with id {:?}", id);
        }
        _ => {
            log::error!("Invalid status {:?} for job with id {:?}. Cannot process.", id, job.status);
            return Err(eyre!("Invalid status {:?} for job with id {:?}. Cannot process.", id, job.status));
        }
    }
    // this updates the version of the job. this ensures that if another thread was about to process
    // the same job, it would fail to update the job in the database because the version would be outdated
    config.database().update_job_status(&job, JobStatus::LockedForProcessing).await?;

    let job_handler = get_job_handler(&job.job_type);
    let external_id = job_handler.process_job(config, &job).await?;

    let metadata = increment_key_in_metadata(&job.metadata, JOB_PROCESS_ATTEMPT_METADATA_KEY)?;
    config
        .database()
        .update_external_id_and_status_and_metadata(&job, external_id, JobStatus::PendingVerification, metadata)
        .await?;

    add_job_to_verification_queue(job.id, Duration::from_secs(job_handler.verification_polling_delay_seconds()))
        .await?;

    Ok(())
}

pub async fn verify_job(id: Uuid) -> Result<()> {
    let config = config().await;
    let job = get_job(id).await?;

    match job.status {
        JobStatus::PendingVerification => {
            log::info!("Verifying job with id {:?}", id);
        }
        _ => {
            log::error!("Invalid status {:?} for job with id {:?}. Cannot verify.", id, job.status);
            return Err(eyre!("Invalid status {:?} for job with id {:?}. Cannot verify.", id, job.status));
        }
    }

    let job_handler = get_job_handler(&job.job_type);
    let verification_status = job_handler.verify_job(config, &job).await?;

    match verification_status {
        JobVerificationStatus::VERIFIED => {
            config.database().update_job_status(&job, JobStatus::Completed).await?;
        }
        JobVerificationStatus::REJECTED => {
            config.database().update_job_status(&job, JobStatus::VerificationFailed).await?;

            // retry job processing if we haven't exceeded the max limit
            let process_attempts = get_u64_from_metadata(&job.metadata, JOB_PROCESS_ATTEMPT_METADATA_KEY)?;
            if process_attempts < job_handler.max_process_attempts() {
                log::info!(
                    "Verification failed for job {}. Retrying processing attempt {}.",
                    job.id,
                    process_attempts + 1
                );
                add_job_to_process_queue(job.id).await?;
                return Ok(());
            }
        }
        JobVerificationStatus::PENDING => {
            log::info!("Inclusion is still pending for job {}. Pushing back to queue.", job.id);
            add_job_to_verification_queue(
                job.id,
                Duration::from_secs(job_handler.verification_polling_delay_seconds()),
            )
            .await?;
        }
    };

    Ok(())
}

fn get_job_handler(job_type: &JobType) -> Box<dyn Job> {
    match job_type {
        JobType::DataSubmission => Box::new(da_job::DaJob),
        _ => unimplemented!("Job type not implemented yet."),
    }
}

async fn get_job(id: Uuid) -> Result<JobItem> {
    let config = config().await;
    let job = config.database().get_job_by_id(id).await?;
    match job {
        Some(job) => Ok(job),
        None => {
            log::error!("Failed to find job with id {:?}", id);
            Err(eyre!("Failed to process job with id {:?}", id))
        }
    }
}

fn increment_key_in_metadata(metadata: &HashMap<String, String>, key: &str) -> Result<HashMap<String, String>> {
    let mut new_metadata = metadata.clone();
    let attempt = metadata.get(key).unwrap_or(&"0".to_string()).parse::<u64>()?;
    new_metadata.insert(key.to_string(), (attempt + 1).to_string());
    Ok(new_metadata)
}

fn get_u64_from_metadata(metadata: &HashMap<String, String>, key: &str) -> Result<u64> {
    Ok(metadata.get(key).unwrap_or(&"0".to_string()).parse::<u64>()?)
}
