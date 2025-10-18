use std::collections::{HashMap, VecDeque};
use std::f32::INFINITY;
use std::fmt;
use std::fmt::Debug;

pub type Core = usize;
pub type GB = usize;
pub type Id = u64;
pub type JobId = u64;
pub struct DominantShare(JobId, f32);
pub type DominantSharePerJob = f32; // Most dominant resource i.e bottleneck resource per job
pub struct Resource {
    cpu: Core,
    mem: GB,
}

pub struct Task<Args: Clone, R> {
    name: &'static str,
    cpu: Core,  // CPU needed to run task
    memory: GB, // RAM needed to run task
    args: Args,
    trigger: Box<dyn Fn(Args) -> R>,
}

impl<Args: fmt::Debug + Clone, R> fmt::Debug for Task<Args, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Task")
            .field("name", &self.name)
            .field("cpu", &self.cpu)
            .field("memory", &self.memory)
            .field("args", &self.args)
            .field("trigger", &"<Fn>") // Closures cannot be printed, so we use a placeholder
            .finish()
    }
}

pub trait TaskTrait: Debug {
    fn run(&mut self);
    fn get_cpu(&self) -> Core;
    fn get_ram(&self) -> GB;
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
    fn get_cpu(&self) -> Core {
        self.cpu
    }
    fn get_ram(&self) -> GB {
        self.memory
    }
}

// #[derive(Debug)]
pub struct Job {
    id: Id,
    tasks_run: i32,
    total_cpu: Core,
    total_memory: GB,
    cpu_usage: Core,
    ram_usage: GB,
    completed: bool,
    dominant_share: DominantSharePerJob, // TODO: This should also reveal the type of resource (RAM/CPU) for debugging purpose
    tasks: VecDeque<Box<dyn TaskTrait>>,
}

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
        writeln!(f, "  Dominant Share: {:.2} \n", self.dominant_share)?;
        writeln!(f, "  Completed: {} \n", self.completed)?;
        writeln!(f, "  Task Count: {} \n", self.tasks.len())?;
        Ok(())
    }
}

impl Job {
    fn new(id: Id, total_cpu: Core, total_memory: GB) -> Self {
        Job {
            id,
            tasks_run: 0, // Number of tasks ran for this job
            cpu_usage: 0, //  CPU usage for this job
            ram_usage: 0, //  RAM usage for this job
            dominant_share: 0.0,
            total_cpu,    // Overall CPU capacity
            total_memory, // Overall Memory capacity
            tasks: VecDeque::new(),
            completed: false,
        }
    }
    fn prepare_resource(&mut self) -> Result<(), ()> {
        match self.tasks.front() {
            Some(t) => {
                // Increase CPU and RAM usage
                self.cpu_usage += t.get_cpu();
                self.ram_usage += t.get_ram();

                // Calculate dominant share for this job
                // TODO: Specify the type of resource
                self.dominant_share = f32::max(
                    self.cpu_usage as f32 / self.total_cpu as f32,
                    self.ram_usage as f32 / self.total_memory as f32,
                );
                Ok(())
            }
            None => {
                self.completed = true;
                println!(
                    "Job completed ID {}, Task Scheduled  [{}]",
                    self.id, self.tasks_run
                );
                Err(())
            }
        }
    }

    fn run_next_task(&mut self) {
        if self.tasks.is_empty() {
            self.completed = true;
            println!(
                "Job completed ID {}, Task Scheduled  [{}]",
                self.id, self.tasks_run
            );
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
            Some(t) => Some(Resource {
                mem: t.get_ram(),
                cpu: t.get_cpu(),
            }),
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
}

impl DRF {
    fn new(total_cpu: Core, total_memory: GB, jobs: HashMap<JobId, Job>) -> Self {
        return DRF {
            total_cpu,
            total_memory,
            jobs,
            cpu_usage: Default::default(),
            ram_usage: Default::default(),
            max_dominant_share: DominantShare(0, 0.0),
            // least_dominant_share: DominantShare(0, 0.0),
        };
    }

    fn schedule(&mut self) {
        loop {
            println!("Here");
            if self.jobs.is_empty() {
                break;
            }
            let (job_id, all_dominant_fair_share_equal) = self.least_dominant_fair_share();
            if all_dominant_fair_share_equal {
                println!(
                    "All dominant fair sharing equal CPU {}, RAM {}",
                    self.cpu_usage, self.ram_usage
                )
            }
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
                                match job.prepare_resource() {
                                    Ok(()) => {
                                        if job.dominant_share >= self.max_dominant_share.1 {
                                            self.max_dominant_share =
                                                DominantShare(job.id, job.dominant_share);
                                        }
                                        job.run_next_task();
                                    }
                                    Err(()) => {
                                        continue;
                                    }
                                }
                            }
                            if (self.cpu_usage) >= self.total_cpu
                                && (self.ram_usage >= self.total_memory)
                            {
                                println!("Job Metrics {:?}", self.jobs);
                                println!(
                                    "Final Usage - CPU {}, RAM {}",
                                    self.cpu_usage, self.ram_usage
                                );
                                break;
                            }
                        }
                        None => return,
                    }
                }

                None => {
                    println!("Jobs Completed");
                    return;
                } // handle later
            }

            if self.jobs.get_mut(&job_id.clone()).unwrap().tasks.is_empty()
            // || (self.cpu_usage) >= self.total_cpu || (self.ram_usage >= self.total_memory)
            {
                println!("Job {} Removed", job_id);
                self.jobs.remove(&job_id);
            }
            if (self.cpu_usage) >= self.total_cpu && (self.ram_usage >= self.total_memory) {
                break;
            }
        }
    }

    fn least_dominant_fair_share(&self) -> (JobId, bool) {
        let mut least_dominant_share = INFINITY;
        let mut job_id = 1000_000_000;
        let mut all_dominant_fair_share_equal = true;
        let dominant_share = self.jobs.iter().next().unwrap().1.dominant_share;

        self.jobs.iter().for_each(|(j_id, j)| {
            println!("Dominant Share {:?} - ID {}", j.dominant_share, j.id);
            if j.dominant_share < least_dominant_share {
                job_id = *j_id;
                least_dominant_share = j.dominant_share;
            }
            all_dominant_fair_share_equal = dominant_share.eq(&j.dominant_share);
        });
        println!("JOB ID {}", job_id);
        (job_id, all_dominant_fair_share_equal)
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
            cpu: 1,
            memory: 3,
            args: (1, 2),
            trigger: Box::new(add),
        }));
        job1.tasks.push_back(Box::new(Task {
            name: "add",
            cpu: 1,
            memory: 3,
            args: (4, 5),
            trigger: Box::new(add),
        }));
        job1.tasks.push_back(Box::new(Task {
            name: "add",
            cpu: 1,
            memory: 3,
            args: (7, 8),
            trigger: Box::new(add),
        }));
        job1.tasks.push_back(Box::new(Task {
            name: "add",
            cpu: 1,
            memory: 3,
            args: (4, 9),
            trigger: Box::new(add),
        }));
        job1.tasks.push_back(Box::new(Task {
            name: "add",
            cpu: 1,
            memory: 3,
            args: (2, 10),
            trigger: Box::new(add),
        }));

        // Job2
        job2.tasks.push_back(Box::new(Task {
            name: "add",
            cpu: 2,
            memory: 1,
            args: (1, 2),
            trigger: Box::new(add),
        }));

        job2.tasks.push_back(Box::new(Task {
            name: "add",
            cpu: 2,
            memory: 1,
            args: (1, 2),
            trigger: Box::new(add),
        }));

        job2.tasks.push_back(Box::new(Task {
            name: "add",
            cpu: 2,
            memory: 1,
            args: (1, 2),
            trigger: Box::new(add),
        }));

        job2.tasks.push_back(Box::new(Task {
            name: "add",
            cpu: 2,
            memory: 1,
            args: (1, 2),
            trigger: Box::new(add),
        }));

        job2.tasks.push_back(Box::new(Task {
            name: "add",
            cpu: 2,
            memory: 1,
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
