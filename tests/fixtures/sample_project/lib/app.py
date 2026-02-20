import os
from datetime import datetime
from .models import User

MAX_RETRIES = 3
DB_HOST = "localhost"

class Application:
    def __init__(self, name):
        self.name = name

    def run(self):
        print(f"Running {self.name}")

    async def start_server(self):
        pass

class Database:
    def connect(self):
        pass

    def disconnect(self):
        pass

def create_app(name):
    return Application(name)

async def main():
    app = create_app("myapp")
    app.run()
