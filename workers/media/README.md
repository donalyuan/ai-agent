# Media Worker

`media-tasks` 承载 MediaInspection/Derivative 与逐集 ExportJob。真实导出只有在显式配置
`FFMPEG_PATH`、`FFPROBE_PATH` 和 StorageProfile 后执行；缺失时返回
`renderer_unconfigured`，不会回退 Mock renderer 或 Local storage。
