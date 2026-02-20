export interface Config {
    host: string;
    port: number;
}

export const defaultConfig: Config = {
    host: 'localhost',
    port: 3000,
};
