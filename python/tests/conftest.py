import asyncio
from collections.abc import Generator

import pytest


@pytest.fixture(scope="session")
def event_loop() -> Generator[asyncio.AbstractEventLoop, None, None]:
  loop = asyncio.new_event_loop()
  yield loop
  loop.close()
