import uvicorn
import yaml
from pathlib import Path

from miloco_server.config import SERVER_CONFIG
from miloco_server.main import app
from miloco_server.utils.normal_util import get_uvicorn_log_config
from miot.client import MIoTClient


def allow_wangwang_camera_model():
    config_path = Path("/app/miot_kit/miot/configs/camera_extra_info.yaml")
    data = yaml.safe_load(config_path.read_text(encoding="utf-8"))
    data.setdefault("denylist", {}).setdefault("camera", {}).pop("chuangmi.camera.029a02", None)
    config_path.write_text(
        yaml.safe_dump(data, allow_unicode=True, sort_keys=False),
        encoding="utf-8",
    )


async def get_devices_with_shared_homes(self, home_list=None, fetch_share_home=False):
    """Use Xiaomi shared homes and avoid an upstream dict-iteration bug.

    The camera family is exposed as a shared home on this account. Upstream
    `MIoTClient.get_devices_async` defaults to `fetch_share_home=False`, then
    mutates a dictionary while iterating when a later shared-home refresh adds
    devices. Miloco only needs cloud device metadata for this bridge, so using
    the HTTP client directly is enough and keeps the list honest.
    """
    devices = await self._http_client.get_devices_async(
        home_infos=home_list,
        fetch_share_home=True if home_list is None else fetch_share_home,
    )
    self._device_buffer = devices
    return self._device_buffer


MIoTClient.get_devices_async = get_devices_with_shared_homes
allow_wangwang_camera_model()


def main():
    uvicorn.run(
        app,
        host=SERVER_CONFIG["host"],
        port=SERVER_CONFIG["port"],
        log_level=SERVER_CONFIG["log_level"],
        log_config=get_uvicorn_log_config(),
    )


if __name__ == "__main__":
    main()
