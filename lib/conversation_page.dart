import 'dart:math' as math;
import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import 'theme.dart';
import 'util.dart';

enum Sender {
  other,
  self,
}

class ConversationPage extends StatelessWidget {
  const ConversationPage({
    super.key,
    required this.name,
    required this.lastMessage,
  });

  final String name;
  final String lastMessage;

  @override
  Widget build(BuildContext context) {
    final random = math.Random();
    return Scaffold(
      appBar: getConversationAppBar(context, name),
      body: Column(
        children: [
          Expanded(
            child: ListView.builder(
              itemBuilder: (context, index) {
                return Message(
                  message:
                      lastMessage.substring(random.nextInt(lastMessage.length)),
                  time: DateTime.now(),
                  sender: random.nextBool() ? Sender.other : Sender.self,
                );
              },
            ),
          ),
          const Text("hello"),
        ],
      ),
    );
  }
}

class Message extends StatelessWidget {
  const Message({
    super.key,
    required this.message,
    required this.time,
    required this.sender,
  });
  final String message;
  final DateTime time;
  final Sender sender;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final double leftPadding;
    final double rightPadding;
    final Color bubbleColor;
    final Alignment alignment;
    if (sender == Sender.self) {
      leftPadding = 36;
      rightPadding = 14;
      bubbleColor = Colors.indigoAccent.shade400;
      alignment = Alignment.topRight;
    } else {
      leftPadding = 14;
      rightPadding = 36;
      bubbleColor = Colors.grey.shade800;
      alignment = Alignment.topLeft;
    }
    return Container(
      margin: EdgeInsets.only(
          left: leftPadding, right: rightPadding, top: 10, bottom: 10),
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(20),
        color: bubbleColor,
      ),
      padding: const EdgeInsets.all(16),
      child: RichText(
        text: TextSpan(
          text: message,
          style: theme.textTheme.bodySmall!.copyWith(
            color: Colors.white,
            fontSize: 24,
          ),
          children: [
            TextSpan(
              text: DateFormat.jm().format(time),
              style: theme.textTheme.bodySmall,
            ),
          ],
        ),
      ),
    );
  }
}

enum ConversationPopupItem { search }

AppBar getConversationAppBar(BuildContext context, String name) {
  final theme = AppBarTheme.of(context);
  return AppBar(
    title: Row(
      children: [
        Padding(
          padding: const EdgeInsets.only(right: 16.0),
          child: CircleAvatar(
            backgroundColor: randomColor(),
            child: Text(name.substring(0, 2)),
          ),
        ),
        Expanded(
            child: Row(
          children: [
            Text(name,
                overflow: TextOverflow.fade,
                style: theme.titleTextStyle!.copyWith(
                  fontSize: 30,
                  color: Colors.white,
                )),
          ],
        )),
        const Icon(
          Icons.account_circle,
        ),
      ],
    ),
    toolbarHeight: theme.toolbarHeight,
    backgroundColor: theme.backgroundColor,
    actions: <Widget>[
      const StubIconButton(icon: Icons.videocam_outlined, name: 'Video Call'),
      const StubIconButton(icon: Icons.phone_outlined, name: 'Call'),
      PopupMenuButton<ConversationPopupItem>(
        onSelected: (ConversationPopupItem item) {},
        iconSize: 36,
        itemBuilder: (BuildContext context) => [],
      ),
    ],
  );
}
