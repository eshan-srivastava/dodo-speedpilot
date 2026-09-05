# AI Usage

This project uses AI-assisted development and content generation tools.

## Scope of AI Usage

- **Code & Logic:** Human-authored and reviewed. AI was used for debugging assistance, syntax reference and generating boilerplate code. Spec Driven development was used to drive significant amounts of code when needed.
- **Documentation:** All base text was written by me in markdown or text files. AI assisted in prettifying and formatting the content.
- **Accuracy:** While I review all AI-assisted outputs, probabilistic errors or hallucinations may occasionally remain but I have done my best to rectify them. Please reach out or raise an issue if you find something that slipped by.

## Decisions takent against or independently

1. Deciding that API key will be split - Key ID and Key Secret similar to AWS. AI recommended stripe style single key but I have seen the the split pattern in razorpay and AWS. This also helps in finding the key quickly in DB. Stripe relies on embedding unique business key info in the api key to lookup fast, its easier to split for my scale. Confirmed my hypothesis with AI after this.

## Things that AI got wrong

1. Recommending Authorization header

## Model Wise Details

### ChatGPT

1. Spec formatting and augmentation with sections like models, must haves.
2. Various options I have for API Key generation and storage options.

### Gemini Web

1.

### Claude Web

1. Spec formatting for topology, do not build, mock PSP
2.

### Open Weights

1.

## Responsibility

The human author(s) remain solely responsible for the content, accuracy, integrity, and fitness-for-purpose of this project.
