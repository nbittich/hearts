const appDiv = document.getElementById("app");
const wsEndpoint = appDiv.dataset.wsEndpoint;
const roomId = appDiv.dataset.roomId;
const currentUserId = appDiv.dataset.userId;
const ws = new WebSocket(`${wsEndpoint}/${roomId}`);

ws.onopen = sendGetCurrentState;

ws.onmessage = function(evt) {
  const roomMessage = JSON.parse(evt.data);
  console.log(roomMessage);

  if (roomMessage.msgType instanceof String) {
    console.log("roomMessage is a string type ", roomMessage.msgType);
  } else {
    if (roomMessage.msgType.waitingForPlayers) {
      renderWaitingForPlayers(roomMessage.msgType.waitingForPlayers);
    }
    else if (roomMessage.msgType.joined) {
      renderPlayerJoined(roomMessage.msgType.joined);

    }

  }
};

ws.onclose = function() {
  // websocket is closed.
  console.log("Connection is closed...");
};




// helper send msg
function sendGetCurrentState() {
  sendStringMessageType("getCurrentState");
}
function sendJoin() {
  sendStringMessageType("join");
}
function sendStringMessageType(msgType) {
  ws.send(JSON.stringify({
    msgType: msgType
  }));
}


// html render

const renderPlayer = (playersDiv, player) => {
  let p = document.createElement("p");
  p.textContent = `${currentUserId === player ? "You" : "Player " + player} joined`;
  p.dataset.userId = player;
  playersDiv.appendChild(p);
};
function renderWaitingForPlayers(players) {
  // update screen with waiting for players

  let playersDiv = appDiv.querySelector("#players");

  if (playersDiv) {
    playersDiv.innerHTML = "";
  } else {
    playersDiv = document.createElement("div");
    playersDiv.id = "players";
    appDiv.appendChild(playersDiv);
  }

  let playerAlreadyJoined = players.includes(currentUserId);

  for (const player of players) {
    if (player)
      renderPlayer(playersDiv, player);
    else {
      let a = document.createElement("a");
      a.href = "#join";
      a.textContent = "Join";
      a.className = "d-block";
      if (!playerAlreadyJoined) {
        a.onclick = sendJoin;
      }
      playersDiv.appendChild(a);
    }
  }

}
function renderPlayerJoined(player) {
  let playersDiv = appDiv.querySelector("#players");
  if (!playersDiv) {
    sendGetCurrentState();
  } else {
    const slot = [...playersDiv.children].find((child) => {
      return child.dataset.userId == null;
    });
    if (slot) {
      console.log("slot found", slot);
      playersDiv.removeChild(slot);
      renderPlayer(playersDiv, player);
    }
  }

}
