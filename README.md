# ICSSC Discord Bot

![ICSSC Bot Banner](./banner.png)

A utility bot created for ICSSC's Discord server. *Massive thanks to the ICSSC Graphics Committee for being awesome and drawing up custom assets in line with ICSSC and UCI's graphical themes.*

## Discord Server Setup

First, invite the bot to the server by using the link logged to the console when the bot starts.

### Restrict Command Usage

It's important to restrict command usage so that non-board members cannot alter attendance, create short URLs, etc. However, it shouldn't be necessary to do this every year since the bot is in a Discord Developer Team (and can thus be transferred).

1. Go to Server Settings > Apps/Integrations > ICS Student Council Set the default
command permission for the bot to deny @everyone, allow @board, and deny All Channels.
2. For `/attended` and `/checkin`, click on their command in the list below, and set it
to allow @everyone and allow `#internal-general`. Discord permissions require both the user
and channel to be allowed, so this enables the command only for users that can message in
`#internal-general`.
3. For `/spottings`, do the same as above, but also allow the command to be used in
`#icssc-spottings`.

Note that the above commands can also be used in the bot's DMs.
The rest of the commands are intended to be used by board members only, and are restricted to
being used within servers only.
Since admins of any of the bot's servers can use the bot's commands, it's important that the
bot is private, meaning only the bot developers can add the bot to a server.

How to make the bot private:
1. Go to https://discord.com/developers/applications
2. Choose "ICS Student Council"
3. Go to "Installation".
Ensure that **only** Guild Install is checked, and that install link is "None".
4. Go to "Bot", and make sure "Public Bot" is toggled *off*.

### Check Permissions

For the following channels, make sure the bot can view the channel and send messages:
- `#internal-general`
- `#socials-info`
- `#matchy-meetups`
- `#icssc-spottings`
- `#bits-and-bytes`
- `#bot-log`

The bot should show at the bottom of the online member list for each of these channels.

## Features & Usage

### Attendance

**Internal Members:** Check in to an event by using the `/checkin` command in `#internal-general` or bot DMs.
Check which events you've attended by using `/attended` in `#internal-general` or DMs.

**Board Members:** Right click a message and choose "Apps > ICSSC Bot > Log Attendance" to
count an event (e.g. planned team social) for everyone mentioned in the message.

### Bits & Bytes

**Board Members:** Right click a message and choose "Log B&B Meetup" on a message.
Then, fill out the fam name and choose the appropriate hangout type.
If the message is sent by a byte, the fam name should populate automatically.

### Matchy Meetups

**Board Members:** Create a Matchy Meetup pairing by running `/matchy create` in `#bot-log`.
Review these pairings, and use `/matchy send` with the provided seed to the pairings.

### Roster Syncing

**Board Members:** Check if anyone's roles are out of sync with the roster using
`/roster check_discord_roles`.
Check if Shared Drive permissions out of sync with the roster by using
`/roster check_google_access`.

### Spotting Logs

We track spottings (both "snipes" and "socials") using the ICSSC Discord bot.
See `#socials-info` for more details on what the differences are, and how to opt in/out.

**Internal Members:** View the spottings leaderboard with `/spottings leaderboard` or
snipe history for the current school year with `/spottings history`

**Board Members:** Right click a message and choose "Log Spotting" to log both snipes
and (unofficial) socials.
Sometimes, pings are not a part of the same message as the image, in which case you should
copy the User ID of the ping, then paste it in to the "who was spotted" field.
It is recommended to log the message with the image rather than a ping because
`/spottings history` will provide a link to the message being logged.

### Short Link Creation

**Board Members:** Create icssc.link short URLs with `/shortlink create`.
If you need to check where a certain shortlink redirects to, use `/shortlink check`.

## Local Development

### Setup

1. Clone the repo
2. `cargo install`
3. Set environment variables based on `.env.example`
4. `cargo run`

### Creating Database Migrations

- `sea-orm-cli migrate generate [name]`
- `sea-orm-cli migrate up`
- `sea-orm-cli generate entity -o entity/src/entities`
    - Revert the removed line in `entities/mod.rs` for the materialized view :P

## Todos
- consider additional helper methods for Roster struct
