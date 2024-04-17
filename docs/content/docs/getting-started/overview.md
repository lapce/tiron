+++
template = "docs/section.html"
title = "overview"
weight = 2
+++

### Installation

Run below to install latest Tiron binary to ```/usr/local/bin```

```bash
curl -sL https://tiron.run/install.sh | sh
```

### Usage

To run a Tiron runbook

```bash
$ tiron run
```

It will run `main.tr` in the current directory.
You can also give a path of the runbook you want to run.

```bash
$ tiron run folder/subfolder/production.tr
```

You can also pre validates the runbook without actually running it by using `check`
which takes the same input as `run`

```bash
$ tiron check
```

### Runbook

The center of Tiron is a runbook. A runbook is a set of settings and actions
for Tiron to know what and how to run things on your remote machines.

#### HCL

For Tiron runbook, we use [HCL](https://github.com/hashicorp/hcl) as the configuration
language.

### Simple Runbook Example

We'll start with a very simple runbook for you to get familiar with the concepts
of Tiron runbooks.

#### group

Before everything, we need to know what remote machines to run actions on, and that's
the `group` block in Tiron. E.g. you want to have a "webservers" group with remote machines
"web1", "web2", and "web3", you'll define it as follows:

```tcl
group "webservers" {
    host "web1" {}
    host "web2" {}
    host "web3" {}
}
```

A group can contain host and other groups at the same time:

```tcl
group "production" {
    group "webservers" {}
    host "db1" {}
}
```

You can define variables in group or host level

```tcl
group "production" {
    group "webservers" {
        group_var = "webservers_group_var"
    }
    host "db1" {
        host_var = "host_var"
    }
    group_production_var = "group_production_var"
}
```

#### run

Now we know what remote machines we'll use,
we can start to run things on them. To do that,
you simply have a `run` block on a `group` you defined earlier:

```tcl
run "production" {
}
```

For things we want to run the remote machines, we call it `action` in Tiron.
And the following run a "copy" `action` which copies `src_file` from local
to `/tmp/dest_path` on the remote machines.

```tcl
run "production" {
    action "copy" {
        params {
            src =  "src_file"
            dest =  "/tmp/dest_path" 
        }
    }
}
```

You can have as many as actions you want in a `run`

```tcl
run "production" {
    action "action1" {}
    action "action2" {}
    action "action1" {}
}
```

#### job

You might have a set of actions you want to reuse in different runs.
`job` would be useful here. A job is defined as a set of actions
that you can use in a `run`. To define a job, you give it a name
and the set of actions it contains. And you can also include another job in a job.

```tcl
job "job1" {
    action "action1" {}
    action "action2" {}
}

job "job2" {
    action "action1" {}
    action "action2" {}
    action "job" {
        params {
            name = "job1"
        }
    }
}
```

Now you can use `job` in your `run`

```tcl
run "production" {
    action "action1" {}
    action "action2" {}
    action "action1" {}
    action "job" {
        params {
            name = "job2"
        }
    }
}
```

#### use

You might want to use a `group` or `job` from another runbook. And `use` can be used to
import them.

```tcl
use "folder/another_runbook.tr" { 
  job "job1" {}
  group "group1" {}
} 
```

You can use `as` to bind the imports to a different name. It would be useful if
you have defined a job or group in your runbook with the same name.

```tcl
use "folder/another_runbook.tr" { 
  job "job1" {
      as = "another_job_name"
  }
  group "group1" {
      as = "another_group_name"
  }
} 

group "group1" {
    host "machine1" {}
}

job "job1" {
    action "action1" {}
    action "action2" {}
}

run "group1" {
    action "job" {
        params {
            name = "another_job_name"
        }
    }
}

run "another_group_name" {
    action "job" {
        params {
            name = "job1"
        }
    }
}
```

These are pretty much all the components in Tiron for you to write your runbooks.
The next thing you'll want to check out is the list of `action` we include in Tiron.
You can view the action docs [here](/docs/actions/command/) or via the tiron command in the console

```bash
$ tiron action
$ tiron action copy
```

There's also some example runbooks at [https://github.com/lapce/tiron/blob/main/examples/example_tiron_project](https://github.com/lapce/tiron/blob/main/examples/example_tiron_project)
