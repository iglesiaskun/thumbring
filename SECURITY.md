# Security policy

## Supported versions

Only the latest release gets security fixes.

| Version | Supported |
|---|---|
| Latest release | Yes |
| Older releases | No |

## Reporting a vulnerability

Please do not report security problems in public issues.

Report them privately on GitHub instead: open the **Security** tab of the
repository and click **Report a vulnerability**
([direct link](https://github.com/iglesiaskun/thumbring/security/advisories/new)). Only
the maintainers can see the report.

Please include:

- what the problem is and what someone could do with it;
- the steps to reproduce it;
- the Thumbring version and the Windows version;
- any idea you have for a fix.

Thumbring is maintained in spare time. We aim to answer within 7 days. Once
the problem is confirmed, we work on the fix with you, publish a release and a
security advisory, and credit you, unless you prefer to stay anonymous.

## Scope

Thumbring runs with the rights of the person who starts it. It watches the
mouse with a low-level hook, simulates key presses and runs the commands
written in its settings file. It makes no network connections. Reports like
these are welcome:

- a way for another program, a file or a web page to make Thumbring run
  commands or simulate input;
- a way for a program without administrator rights to use a Thumbring that
  has them (see "Start with Windows as administrator" in the README) to do
  something with administrator rights, other than replacing `thumbring.exe`
  in a folder that program can write to, which the README already warns
  about;
- a release file that does not match the source code, or a weakness in the
  release workflow.

Out of scope: what a command in your own settings file does. The file belongs
to you, and a command can open anything you could open yourself.
