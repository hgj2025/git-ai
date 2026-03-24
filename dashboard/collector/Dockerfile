FROM python:3.12-slim

# git 是核心依赖
RUN apt-get update && apt-get install -y --no-install-recommends \
    git \
    openssh-client \
    && rm -rf /var/lib/apt/lists/*

# SSH 配置：接受新 host key，不交互
RUN mkdir -p /root/.ssh && \
    echo "StrictHostKeyChecking accept-new" >> /root/.ssh/config && \
    echo "BatchMode yes" >> /root/.ssh/config

WORKDIR /app
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

COPY . .

CMD ["python", "main.py"]
