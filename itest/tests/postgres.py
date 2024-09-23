import docker
import logging
import psycopg2
import time

from pyln.testing.utils import (
    drop_unused_port,
    reserve_unused_port,
)


class PostgresContainer(object):
    def __init__(
        self,
        container,
        port,
        db_name,
    ):
        self.container = container
        self.port = port
        self.db_name = db_name
        self.connectionstring = "postgres://postgres:POSTGRES_PASSWORD@127.0.0.1:{}/postgres?sslmode=disable".format(
            port
        )

    def start(self, timeout=30):
        # Wait for the container to start
        start = time.time()
        logging.debug("starting postgres container")
        self.container.start()
        logging.debug("started postgres container")
        while self.container.health != "healthy":
            self.container.reload()
            elapsed = time.time() - start
            if elapsed > timeout:
                raise TimeoutError("Postgres container did not start in time")

        logging.debug("postgres container is healthy")
        # Wait for the database to be ready
        start = time.time()
        while not self.is_postgres_available():
            elapsed = time.time() - start
            if elapsed > timeout:
                raise TimeoutError("Postgres database did not start in time")

        logging.debug("postgres container is available")

    def stop(self):
        self.container.stop()

    def is_postgres_available(self):
        try:
            with psycopg2.connect(
                dbname=self.db_name,
                user="postgres",
                password="POSTGRES_PASSWORD",
                host="localhost",
                port=self.port,
            ) as conn:
                with conn.cursor() as cur:
                    cur.execute("SELECT 1")
                return True
        except psycopg2.OperationalError:
            return False


class PostgresContainerFactory(object):
    def __init__(self, testname):
        self.testname = testname
        self.reserved_ports = []
        self.containers = []

    def get_container(self):
        port = reserve_unused_port()
        client = docker.from_env()
        client.images.pull("postgres:16")
        db_name = "swapd"
        container = client.containers.create(
            image="postgres:16",
            auto_remove=True,
            name="swapd-test-{}".format(self.testname),
            ports={f"5432/tcp": port},
            environment={
                "POSTGRES_PASSWORD": "POSTGRES_PASSWORD",
                "POSTGRES_DB": db_name,
            },
            detach=True,
            healthcheck={
                "test": ["CMD-SHELL", "pg_isready -U postgres"],
                "interval": 1_000_000_000,
                "timeout": 1_000_000_000,
                "retries": 10,
            },
        )

        self.reserved_ports.append(port)
        postgres_container = PostgresContainer(container, port, db_name)
        self.containers.append(postgres_container)
        postgres_container.start()
        return postgres_container

    def killall(self):
        err_msgs = []
        for i in range(len(self.containers)):
            try:
                self.containers[i].stop()
            except Exception as e:
                err_msgs.append("failed to stop postgres container: {}".format(str(e)))

        for p in self.reserved_ports:
            drop_unused_port(p)

        return err_msgs
