"""Convierte los dicts de opciones de yt-dlp (formato libreria python)
a una lista de argumentos de linea de comandos para yt-dlp.exe.
"""

POSTPROCESSOR_MAP = {
    "FFmpegExtractAudio": lambda pp: [
        "--extract-audio",
        "--audio-format", pp.get("preferredcodec", "mp3"),
    ] + (["--audio-quality", str(pp["preferredquality"])] if pp.get("preferredquality") else []),
    "SponsorBlock": lambda pp: ["--sponsorblock-remove", ",".join(pp.get("categories", []))],
    "FFmpegMetadata": lambda pp: ["--add-metadata"],
    "EmbedThumbnail": lambda pp: ["--embed-thumbnail"],
    "ModifyChapters": lambda pp: [],  # ya cubierto por --sponsorblock-remove
}


def opts_to_cli_args(opts: dict) -> list:
    args = []

    if opts.get("format"):
        args += ["-f", opts["format"]]
    if opts.get("merge_output_format"):
        args += ["--merge-output-format", opts["merge_output_format"]]
    if opts.get("windowsfilenames"):
        args += ["--windows-filenames"]
    if opts.get("ffmpeg_location"):
        args += ["--ffmpeg-location", opts["ffmpeg_location"]]
    if opts.get("ratelimit"):
        args += ["--limit-rate", str(opts["ratelimit"])]
    if opts.get("proxy"):
        args += ["--proxy", opts["proxy"]]
    if opts.get("cookiefile"):
        args += ["--cookies", opts["cookiefile"]]
    if opts.get("cookiesfrombrowser"):
        val = opts["cookiesfrombrowser"]
        browser = val[0] if isinstance(val, (list, tuple)) else val
        args += ["--cookies-from-browser", browser]
    if opts.get("writethumbnail"):
        args += ["--write-thumbnail"]
    if opts.get("writesubtitles"):
        args += ["--write-subs"]
    if opts.get("writeautomaticsub"):
        args += ["--write-auto-subs"]
    if opts.get("subtitleslangs"):
        args += ["--sub-langs", ",".join(opts["subtitleslangs"])]
    if opts.get("sleep_interval") is not None:
        args += ["--sleep-interval", str(opts["sleep_interval"])]
    if opts.get("max_sleep_interval") is not None:
        args += ["--max-sleep-interval", str(opts["max_sleep_interval"])]
    if opts.get("retries"):
        args += ["--retries", str(opts["retries"])]
    if opts.get("continuedl", True):
        args += ["--continue"]

    pp_args = opts.get("postprocessor_args", {})
    if pp_args.get("ffmpeg"):
        ffmpeg_args = " ".join(pp_args["ffmpeg"])
        args += ["--postprocessor-args", f"ffmpeg:{ffmpeg_args}"]

    for pp in opts.get("postprocessors", []):
        key = pp.get("key")
        fn = POSTPROCESSOR_MAP.get(key)
        if fn:
            args += fn(pp)

    return args
