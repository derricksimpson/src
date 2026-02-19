# Plan for implementing src CLI
We are building src -  an amazing command line interface / tool for retrieving all sorts of details from source code.
We are building in C#, with AOT compilation and single file deployment. Optimized for speed and memory usage at every possible level.


Assuming we have completed our work, this is what src looks like:

Here are the key features:

src can return a hierarchy of all folders which contain source code files (quickly showing the outline of the project).

src can return a heirarcy of folder and file names (including those with filename pattern matches)

src uses mass parallelizaton and memory mapped file loading to also allow for searching within the source fils themselves for specific strings or regular expression patterns. 


src returns a YAML based structured response of multiple file chunks all in one go, even with sections and parts of a file, along with line numbers intact!

src help returns details about the command and options.

In this example, we are searching for the word "Payments" in all TS files and returning the results in a YAML format while padding 2 lines before/after the matching lines.  Timeout of 5 seconds, ensures no long running tasks are allowed.

Example
```bash
src --r *.ts --f Payments|Payment --pad 2 --timeout 5
```

returns

```yaml
files:
  - path: src/components/payments/index.ts
  - contents: |

    1.  import { Payments } from './payments';
    2. 
    3.  export const Payments = () => {
    4.    return <div>Payments</div>;
    5.  };
  - path: src/components/payments/payments.ts
  - content: |
    more contents here
```


## Plan
Using C# Best practices and patterns:
Create a proper project structure with proper namespaces and class/method/property names.
do not use non MIT or similar licenses, but use dependencies if needed.

Implement the core CLI and command line arguments.
Implement the file/filter hierarchy retrieval by filename.
Implement file loading with Memory mapping and parallelization.


Add more options and features that fit exactly with this plan as you see fit!