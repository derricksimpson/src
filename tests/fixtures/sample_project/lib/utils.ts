import { Config } from './config';
const express = require('express');

export function greet(name: string): string {
    return `Hello, ${name}!`;
}

export class UserService {
    private users: string[] = [];

    public addUser(name: string): void {
        this.users.push(name);
    }

    async fetchUser(id: number): Promise<string> {
        return this.users[id];
    }
}

export interface ILogger {
    log(message: string): void;
}

export type UserID = string;

export enum LogLevel {
    DEBUG,
    INFO,
    WARN,
    ERROR,
}

export const MAX_USERS = 100;

const handler = (req: any, res: any) => {
    res.send('ok');
};
