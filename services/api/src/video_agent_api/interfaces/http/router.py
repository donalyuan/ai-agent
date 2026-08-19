"""Router compatibility export."""

from .assets import router as assets_router
from .projects_episodes import router

__all__ = ["assets_router", "router"]
