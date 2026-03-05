import { UserService } from './utils';

describe('UserService', () => {
    it('adds a user', () => {
        const svc = new UserService();
        svc.addUser('alice');
    });
});
