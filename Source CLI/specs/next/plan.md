src desc uses advanced LLM models to provide a high level overview of the file (while caching the results in a .src/{path} folder), including high level - striaght to the point - succinct summary, and key sections (per specific file type, like JS, TS, C#, etc.).




```bash
src desc --r src/components/payments/index.ts
```
returns
```yaml
file: src/user.ts
language: typescript
hash: 8ab23f

imports:
  - react
  - ./auth

declarations:
  - id: user_class
    type: class
    name: UserService
    startLine: 5
    endLine: 120
    summary: Handles user operations

methods:
  - id: create_user
    parent: user_class
    name: createUser
    signature: createUser(email: string): Promise<User>
    startLine: 20
    endLine: 40
    summary: Creates a new user

  - id: delete_user
    parent: user_class
    name: deleteUser
    startLine: 42
    endLine: 60

exports:
  - UserService
```
