# LeoGPT

A Discord bot that connects to AI models through OpenRouter. Mention the bot in any channel to chat with it.

<img src="assets/icon.png" alt="LeoGPT icon" width="256">

## Features

- Conversational AI via OpenRouter API
- Follows Discord reply chains for context
- Processes images, videos, audio, and PDFs
- Auto-responses triggered by user/message match rules
- Built-in tools for Discord lookups and web search
- Image and audio generation via OpenRouter
- Voice channel music playback with fuzzy song search

## Setup

### Environment Variables

Create a `.env` file:

```env
DISCORD_TOKEN=your_discord_bot_token
OPENROUTER_API_KEY=your_openrouter_api_key
MUSIC_S3_URL=https://your-bucket.s3.your-region.backblazeb2.com/your-prefix/
AWS_ACCESS_KEY_ID=your_s3_access_key_id
AWS_SECRET_ACCESS_KEY=your_s3_secret_access_key
```

### Run with Docker

```bash
docker compose up -d
```

## Usage

Mention the bot in Discord to start a conversation. Reply to its messages to continue the thread.

## License

[LICENSE](LICENSE)
