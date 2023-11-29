import discord
from discord.ext import commands
from openai import AsyncOpenAI
import logging
import asyncio
import time
import config
from collections import defaultdict
from datetime import datetime, timedelta

# Configuration and Constants
OPENAI_API_KEY = config.openai_api_key 
DISCORD_BOT_TOKEN = config.discord_bot_token  
ASSISTANT_ID = config.assistant_id
MESSAGE_CHUNK_SIZE = 2000
THREAD_INACTIVITY_TIMEOUT_HOURS = 1

# Setting up logging
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')

# OpenAI Client Setup
openai_client = AsyncOpenAI(api_key=OPENAI_API_KEY)

# Discord Bot Setup
intents = discord.Intents.default()
bot = commands.Bot(command_prefix="!", intents=intents)

# Thread Management
thread_ids = defaultdict(lambda: {"thread_id": None, "last_used": datetime.now()})

# Function to create a new thread
async def create_new_thread(identifier):
    global thread_ids
    try:
        thread = await openai_client.beta.threads.create()
        thread_ids[identifier] = {"thread_id": thread.id, "last_used": datetime.now()}
        logging.info(f"New thread created for {identifier} with ID: {thread.id}")
    except Exception as e:
        logging.error(f"Error during thread creation for {identifier}: {e}")
        raise

# Function to send messages in chunks
async def send_in_chunks(channel, message):
    logging.info("Sending message in chunks")
    try:
        while message:
            split_index = (message.rfind(' ', 0, MESSAGE_CHUNK_SIZE) + 1) if len(message) > MESSAGE_CHUNK_SIZE else len(message)
            chunk = message[:split_index].strip()
            await channel.send(chunk)
            message = message[split_index:]
        logging.info("All chunks sent successfully")
    except Exception as e:
        logging.error(f"Error sending message in chunks: {e}")
        raise

# Function to send message to OpenAI
async def send_message_to_openai(clean_message, thread_id):
    try:
        await openai_client.beta.threads.messages.create(
            thread_id=thread_id,
            role="user",
            content=clean_message
        )
    except Exception as e:
        logging.error(f"Error sending message to OpenAI: {e}")
        raise

# Function to check OpenAI response
async def check_openai_response(thread_id, run_id):
    try:
        start_time = time.time()
        while True:
            updated_run = await openai_client.beta.threads.runs.retrieve(
                thread_id=thread_id,
                run_id=run_id
            )
            if updated_run.status == "completed":
                break

            elapsed_time = time.time() - start_time
            sleep_time = min(1 + elapsed_time / 10, 5)
            await asyncio.sleep(sleep_time)
    except Exception as e:
        logging.error(f"Error checking OpenAI response: {e}")
        raise

# Function to retrieve the latest response from OpenAI
async def retrieve_latest_response(thread_id):
    try:
        messages = await openai_client.beta.threads.messages.list(thread_id=thread_id)
        assistant_messages = [msg for msg in messages.data if msg.role == "assistant"]
        if assistant_messages:
            latest_message = max(assistant_messages, key=lambda x: x.created_at)
            return latest_message.content[0].text.value
        else:
            return "No response from the assistant."
    except Exception as e:
        logging.error(f"Error retrieving response from OpenAI: {e}")
        raise

# Function to interact with OpenAI
async def interact_with_openai(clean_message, identifier):
    global thread_ids
    thread_info = thread_ids[identifier]
    thread_id = thread_info["thread_id"]
    if thread_id is None:
        await create_new_thread(identifier)
        thread_id = thread_ids[identifier]["thread_id"]

    try:
        await send_message_to_openai(clean_message, thread_id)

        run = await openai_client.beta.threads.runs.create(
            thread_id=thread_id,
            assistant_id=ASSISTANT_ID
        )

        await check_openai_response(thread_id, run.id)
        return await retrieve_latest_response(thread_id)

    except Exception as e:
        logging.error(f"Error during OpenAI interaction: {e}")
        return "I'm having trouble processing your request right now."

# Function for cleaning up old threads
def cleanup_old_threads():
    now = datetime.now()
    for key, value in list(thread_ids.items()):
        if now - value["last_used"] > timedelta(hours=THREAD_INACTIVITY_TIMEOUT_HOURS):
            del thread_ids[key]

# Bot event: on_ready
@bot.event
async def on_ready():
    try:
        logging.info(f"Logged in as {bot.user.name}")
    except Exception as e:
        logging.error(f"Error in on_ready: {e}")

# Bot event: on_message
@bot.event
async def on_message(message):
    try:
        if message.author == bot.user:
            return

        identifier = message.channel.id 
        thread_ids[identifier]["last_used"] = datetime.now()

        cleanup_old_threads()

        bot_mention = f'<@{bot.user.id}>'
        if message.content.startswith(bot_mention):
            clean_message = discord.utils.remove_markdown(message.clean_content)
            logging.info(f"Received message from {message.author.name}: {clean_message}")

            async with message.channel.typing():
                response = await interact_with_openai(clean_message, identifier)
                logging.info(f"OpenAI response: {response}")
            
            await send_in_chunks(message.channel, response)
    except Exception as e:
        logging.error(f"Error in on_message for {message.content}: {e}")

# Running the bot
bot.run(DISCORD_BOT_TOKEN)
