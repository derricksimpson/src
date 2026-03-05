import { greet } from './utils';

describe('greet', () => {
    it('returns greeting', () => {
        expect(greet('world')).toBe('Hello, world!');
    });
});

function testHelper(): void {
    console.log('test helper');
}
