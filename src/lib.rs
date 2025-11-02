use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::fmt::Debug;

pub type Core = usize;
pub type GB = usize;
pub type JobId = u64;

#[derive(Clone, Debug, PartialEq)]
pub enum ResourceType {
    CPU,
    RAM,
}
#[derive(Clone, Debug)]
pub struct DominantShare {
    job_id: JobId,
    resource_type: ResourceType,
    resource_share: f32,
}
impl PartialEq for DominantShare {
    fn eq(&self, other: &Self) -> bool {
        self.resource_share == other.resource_share
    }
}
impl PartialOrd for DominantShare {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.resource_share.partial_cmp(&other.resource_share)
    }
}
#[derive(Clone, Debug)]
pub struct Resource {
    cpu: Core,
    mem: GB,
}

pub struct Task<Args: Clone, R> {
    name: &'static str,
    resource_demand: Resource,
    args: Args,
    trigger: Box<dyn Fn(Args) -> R>,
}

pub trait TaskTrait: Debug {
    fn run(&mut self);
    fn get_resource_demand(&self) -> Resource;
}

impl<Args, R> TaskTrait for Task<Args, R>
where
    Args: 'static + Clone + Debug,
    R: 'static,
{
    fn run(&mut self) {
        let args = self.args.clone();
        (self.trigger)(args);
    }
    fn get_resource_demand(&self) -> Resource {
        self.resource_demand.clone()
    }
}

// #[derive(Debug)]
pub struct Job {
    id: JobId,
    tasks_run: i32,
    total_cpu: Core,
    total_memory: GB,
    cpu_usage: Core,
    ram_usage: GB,
    completed: bool,
    dominant_share: DominantShare,
    tasks: VecDeque<Box<dyn TaskTrait>>,
}

impl Job {
    fn new(id: JobId, total_cpu: Core, total_memory: GB) -> Self {
        Job {
            id,
            tasks_run: 0, // Number of tasks ran for this job
            cpu_usage: 0, //  CPU usage for this job
            ram_usage: 0, //  RAM usage for this job
            dominant_share: DominantShare::default(),
            total_cpu,    // Overall CPU capacity
            total_memory, // Overall Memory capacity
            tasks: VecDeque::new(),
            completed: false,
        }
    }
    fn get_dominant_share(&self) -> &DominantShare {
        &self.dominant_share
    }
    fn allocate_resource_for_next_task(&mut self) -> Result<(), ()> {
        match self.tasks.front() {
            Some(t) => {
                // Increase CPU and RAM usage
                let resource_demand = t.get_resource_demand();
                self.cpu_usage += resource_demand.cpu;
                self.ram_usage += resource_demand.mem;

                // Calculate dominant share for this job
                // TODO: Specify the type of resource
                self.dominant_share = DominantShare {
                    job_id: self.id,
                    resource_type: if resource_demand.cpu > resource_demand.mem {
                        ResourceType::CPU
                    } else {
                        ResourceType::RAM
                    },
                    resource_share: f32::max(
                        self.cpu_usage as f32 / self.total_cpu as f32,
                        self.ram_usage as f32 / self.total_memory as f32,
                    ),
                };
                Ok(())
            }
            None => {
                self.completed = true;
                Err(())
            }
        }
    }

    fn run_next_task(&mut self) {
        if self.tasks.is_empty() {
            self.completed = true;
        } else {
            match self.tasks.pop_front() {
                Some(mut t) => {
                    self.tasks_run += 1;
                    t.run();
                }
                None => {}
            }
        }
    }
    fn get_next_task_resource_demand(&self) -> Option<Resource> {
        match self.tasks.front() {
            Some(t) => Some(t.get_resource_demand()),
            None => None,
        }
    }
}

pub struct DRF {
    total_cpu: Core,
    total_memory: GB,
    jobs: HashMap<JobId, Job>,
    max_dominant_share: DominantShare,
    // least_dominant_share: DominantShare,
    cpu_usage: Core,
    ram_usage: GB,
    skip_jobs: HashSet<JobId>,
}

impl DRF {
    fn new(total_cpu: Core, total_memory: GB, jobs: HashMap<JobId, Job>) -> Self {
        return DRF {
            total_cpu,
            total_memory,
            jobs,
            cpu_usage: 0,
            ram_usage: 0,
            max_dominant_share: DominantShare::default(),
            // Jobs that are skipped due to not enough resources
            skip_jobs: HashSet::new(),
            // least_dominant_share: DominantShare(0, 0.0),
        };
    }
    fn update_max_dominant_share(&mut self, job_dominant_share: &DominantShare) {
        if job_dominant_share >= &self.max_dominant_share {
            self.max_dominant_share = DominantShare {
                job_id: job_dominant_share.job_id,
                resource_type: job_dominant_share.resource_type.clone(),
                resource_share: job_dominant_share.resource_share,
            };
        }
    }
    fn increment_resource_usage(&mut self, resource: Resource) {
        self.cpu_usage += resource.cpu;
        self.ram_usage += resource.mem;
    }
    fn schedule(&mut self) {
        loop {
            if self.jobs.is_empty() {
                break;
            }
            let (job_id, _) = self.least_dominant_fair_share();
            match self.jobs.get_mut(&job_id.clone()) {
                Some(job) => {
                    let task_resource = job.get_next_task_resource_demand();
                    match task_resource {
                        Some(resource) => {
                            if ((self.cpu_usage + resource.cpu) <= self.total_cpu)
                                && ((self.ram_usage + resource.mem) <= self.total_memory)
                            {
                                self.cpu_usage += resource.cpu;
                                self.ram_usage += resource.mem;
                                match job.allocate_resource_for_next_task() {
                                    Ok(()) => {
                                        job.run_next_task();
                                        let job_dominant_share = job.get_dominant_share();
                                        if job_dominant_share.ge(&self.max_dominant_share) {
                                            self.max_dominant_share = DominantShare {
                                                job_id: job.id,
                                                resource_type: job_dominant_share
                                                    .resource_type
                                                    .clone(),
                                                resource_share: job_dominant_share.resource_share,
                                            };
                                        }
                                        println!(
                                            "Max Dominant Share {:?}, (Job{:?}, {:?})",
                                            self.max_dominant_share, job.id, job.dominant_share
                                        );

                                        println!(
                                            "Job C-{:?}, R-{:?}",
                                            self.cpu_usage, self.ram_usage
                                        );
                                    }
                                    Err(()) => {
                                        // TODO: Handle this later
                                        continue;
                                    }
                                }
                            } else {
                                // Job is skipped due to not enough resources
                                self.skip_jobs.insert(job_id);
                            }
                        }
                        None => {
                            // Job is skipped due to no more tasks
                            self.skip_jobs.insert(job_id);
                        }
                    }
                }
                None => {
                    self.skip_jobs.insert(job_id);
                    continue;
                } // handle later
            }

            if self.skip_jobs.len() == self.jobs.len() {
                break;
            }
        }
        println!("skip_jobs: {:?}", self.skip_jobs);
        println!("jobs: {:?}", self.jobs);
        println!("max_dominant_share: {:?}", self.max_dominant_share);
        println!("cpu_usage: {:?}", self.cpu_usage);
        println!("ram_usage: {:?}", self.ram_usage);
    }

    fn least_dominant_fair_share(&self) -> (JobId, bool) {
        let mut least_dominant_share = self.max_dominant_share.to_owned();
        let mut job_id = self.max_dominant_share.job_id;
        let mut all_dominant_fair_share_equal = true;
        let dominant_share = self.jobs.iter().next().unwrap().1.get_dominant_share();

        let mut least_share = |j: &Job| {
            let job_dominant_share = j.get_dominant_share();
            if job_dominant_share.lt(&least_dominant_share) {
                job_id = j.id;
                least_dominant_share = job_dominant_share.to_owned();
            }
            all_dominant_fair_share_equal = dominant_share.eq(&job_dominant_share);
        };
        self.jobs.iter().for_each(|j| {
            if !self.skip_jobs.contains(&j.0) {
                least_share(j.1)
            }
        });
        println!("least_dominant_share: {:?}", least_dominant_share);
        println!("job_id: {job_id}");
        (job_id, all_dominant_fair_share_equal)
    }
}
impl Default for DominantShare {
    fn default() -> Self {
        DominantShare {
            job_id: 0,
            resource_type: ResourceType::CPU,
            resource_share: 0.0,
        }
    }
}
// Debug Implementations
impl fmt::Debug for Job {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Job ID: {}\n", self.id)?;
        writeln!(f, "  Tasks Run: {} \n", self.tasks_run)?;
        writeln!(f, "  CPU Usage: {}/{} \n", self.cpu_usage, self.total_cpu)?;
        writeln!(
            f,
            "  RAM Usage: {}/{} \n",
            self.ram_usage, self.total_memory
        )?;
        writeln!(f, "  Dominant Share: {:?} \n", self.dominant_share)?;
        writeln!(f, "  Completed: {} \n", self.completed)?;
        writeln!(f, "  Task Count: {} \n", self.tasks.len())?;
        Ok(())
    }
}

impl<Args: fmt::Debug + Clone, R> fmt::Debug for Task<Args, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Task")
            .field("name", &self.name)
            .field("resource_demand", &self.resource_demand)
            .field("args", &self.args)
            .field("trigger", &"<Fn>") // Closures cannot be printed, so we use a placeholder
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schedule() {
        let cpu = 12;
        let ram = 12;
        let add = |(a, b): (i32, i32)| a + b;
        let mut job1 = Job::new(0, cpu, ram);
        let mut job2 = Job::new(1, cpu, ram);

        job1.tasks.push_back(Box::new(Task {
            name: "add",
            resource_demand: Resource { cpu: 1, mem: 3 },
            args: (1, 2),
            trigger: Box::new(add),
        }));
        job1.tasks.push_back(Box::new(Task {
            name: "add",
            resource_demand: Resource { cpu: 1, mem: 3 },
            args: (4, 5),
            trigger: Box::new(add),
        }));
        job1.tasks.push_back(Box::new(Task {
            name: "add",
            resource_demand: Resource { cpu: 1, mem: 3 },
            args: (7, 8),
            trigger: Box::new(add),
        }));
        job1.tasks.push_back(Box::new(Task {
            name: "add",
            resource_demand: Resource { cpu: 1, mem: 3 },
            args: (4, 9),
            trigger: Box::new(add),
        }));
        job1.tasks.push_back(Box::new(Task {
            name: "add",
            resource_demand: Resource { cpu: 1, mem: 3 },
            args: (2, 10),
            trigger: Box::new(add),
        }));

        // Job2
        job2.tasks.push_back(Box::new(Task {
            name: "add",
            resource_demand: Resource { cpu: 2, mem: 1 },
            args: (1, 2),
            trigger: Box::new(add),
        }));

        job2.tasks.push_back(Box::new(Task {
            name: "add",
            resource_demand: Resource { cpu: 2, mem: 1 },
            args: (1, 2),
            trigger: Box::new(add),
        }));

        job2.tasks.push_back(Box::new(Task {
            name: "add",
            resource_demand: Resource { cpu: 2, mem: 1 },
            args: (1, 2),
            trigger: Box::new(add),
        }));

        job2.tasks.push_back(Box::new(Task {
            name: "add",
            resource_demand: Resource { cpu: 2, mem: 1 },
            args: (1, 2),
            trigger: Box::new(add),
        }));

        job2.tasks.push_back(Box::new(Task {
            name: "add",
            resource_demand: Resource { cpu: 2, mem: 1 },
            args: (1, 2),
            trigger: Box::new(add),
        }));

        let mut jobs = HashMap::new();
        jobs.insert(job1.id, job1);
        jobs.insert(job2.id, job2);
        let mut drf = DRF::new(cpu, ram, jobs);
        let _ = drf.schedule();
    }
}
